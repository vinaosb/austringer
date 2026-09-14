/**
 * The extension's daemon client: one SSE event stream plus stateless
 * command POSTs over the daemon protocol v1, with cursor-exact resume,
 * capped exponential backoff, snapshot resync acceptance, and typed
 * failures for every unhealthy path (daemon absent, auth refused,
 * malformed frames, unknown asks).
 */

import { decodeServerMessage, encodeClient } from "./codec.ts";
import type { HttpIo, PostOutcome, SseOpen, SseStream } from "./http.ts";
import { createSseParser } from "./sse.ts";
import type {
    Answer,
    AskPayload,
    ClientMsg,
    ErrorCode,
    Frame,
    Resolution,
} from "./protocol.ts";

export type ClientStatus = "connecting" | "connected" | "disconnected";

export type ClientEvent =
    | { kind: "status"; status: ClientStatus; attempt: number }
    | { kind: "frame"; frame: Frame }
    | { kind: "resync"; lastEventId: number }
    | { kind: "snapshot"; frames: Frame[] }
    | { kind: "ask"; ask: AskPayload }
    | { kind: "resolved"; askId: number; resolution: Resolution }
    | { kind: "malformed"; detail: string }
    | { kind: "protocol-error"; code: ErrorCode; message: string }
    | { kind: "bye"; reason: string };

export type CommandError =
    | { kind: "auth" }
    | { kind: "unreachable"; detail: string }
    | { kind: "unknown_ask" }
    | { kind: "unknown_session" }
    | { kind: "http"; status: number; body: string }
    | { kind: "out_of_bounds"; field: string };

export type CommandResult = { ok: true } | { ok: false; error: CommandError };

export interface ClientOptions {
    baseUrl: string;
    token: string;
    sessionId: string;
    clientId: string;
    http: HttpIo;
    onEvent: (event: ClientEvent) => void;
    sleep?: (ms: number) => Promise<void>;
    random?: () => number;
    backoff?: { initialMs: number; maxMs: number };
}

export class HarnessClient {
    private readonly options: Required<ClientOptions>;
    readonly token: string;
    private stopped = false;
    private cursor = 0;
    private attempt = 0;
    private livePhase = false;
    private stream: SseStream | null = null;
    private running: Promise<void> = Promise.resolve();
    private currentStatus: ClientStatus = "disconnected";

    constructor(options: ClientOptions) {
        this.token = options.token;
        this.options = {
            ...options,
            sleep: options.sleep ?? ((ms) => new Promise((resolve) => { setTimeout(resolve, ms); })),
            random: options.random ?? Math.random,
            backoff: options.backoff ?? { initialMs: 1000, maxMs: 15000 },
        };
    }

    start(): void {
        this.running = this.run();
    }

    async stop(): Promise<void> {
        this.stopped = true;
        this.stream?.close();
        await this.running;
    }

    lastEventId(): number {
        return this.cursor;
    }

    submit(text: string): Promise<CommandResult> {
        return this.postCommand({ type: "command", command: { type: "submit", text } });
    }

    steer(text: string): Promise<CommandResult> {
        return this.postCommand({ type: "command", command: { type: "steer", text } });
    }

    cancelTurn(): Promise<CommandResult> {
        return this.postCommand({ type: "command", command: { type: "cancel" } });
    }

    answerAsk(askId: number, answer: Answer): Promise<CommandResult> {
        return this.postCommand({ type: "permission_answer", ask_id: askId, answer });
    }

    cancelAsk(askId: number): Promise<CommandResult> {
        return this.postCommand({ type: "permission_cancel", ask_id: askId });
    }

    private emit(event: ClientEvent): void {
        this.options.onEvent(event);
    }

    private async run(): Promise<void> {
        while (!this.stopped) {
            this.attempt += 1;
            this.livePhase = false;
            this.setStatus("connecting");
            const token = encodeURIComponent(this.options.token);
            const after = this.cursor > 0 ? `after=${this.cursor}&` : "";
            const url = `${this.options.baseUrl}/s/${this.options.sessionId}/events?${after}token=${token}`;
            const ended = new Promise<void>((resolve) => {
                this.resolveEnd = resolve;
            });
            const parser = createSseParser((payload) => {
                this.handlePayload(payload);
            });
            const open = await this.options.http.openSse(url, {
                onChunk: (chunk) => {
                    parser.push(chunk);
                },
                onEnd: () => {
                    this.resolveEnd?.();
                },
                onError: () => {
                    this.resolveEnd?.();
                },
            });
            if (!open.ok) {
                if (this.stopped) {
                    return;
                }
                if (open.status === 401 || open.status === 403) {
                    this.setStatus("disconnected");
                    return;
                }
                await this.backoff();
                continue;
            }
            if (this.stopped) {
                // stop() saw no stream yet; close here or `await ended` hangs forever.
                open.stream.close();
                return;
            }
            this.stream = open.stream;
            await ended;
            this.stream = null;
            if (this.stopped) {
                return;
            }
            if (this.currentStatus === "connected") {
                this.setStatus("disconnected");
            }
            await this.backoff();
        }
    }

    private resolveEnd: (() => void) | null = null;

    private setStatus(status: ClientStatus): void {
        this.currentStatus = status;
        this.emit({ kind: "status", status, attempt: this.attempt });
        if (status === "connected") {
            this.attempt = 0;
        }
    }

    private async backoff(): Promise<void> {
        if (this.attempt <= 0) {
            return;
        }
        const { initialMs, maxMs } = this.options.backoff;
        const raw = Math.min(maxMs, initialMs * 2 ** (this.attempt - 1));
        const jittered = raw * (0.8 + 0.4 * this.options.random());
        await this.options.sleep(jittered);
    }

    private handlePayload(payload: string): void {
        const decoded = decodeServerMessage(payload);
        if (!decoded.ok) {
            this.emit({ kind: "malformed", detail: decoded.error.kind });
            return;
        }
        const msg = decoded.value;
        switch (msg.type) {
            case "hello_ack":
                this.cursor = Math.max(this.cursor, msg.last_event_id);
                this.setStatus("connected");
                this.livePhase = true;
                return;
            case "event":
                this.acceptFrame(msg.frame);
                return;
            case "permission_ask":
                this.emit({ kind: "ask", ask: msg.ask });
                return;
            case "permission_resolved":
                this.emit({ kind: "resolved", askId: msg.ask_id, resolution: msg.resolution });
                return;
            case "state_resync_required":
                this.emit({ kind: "resync", lastEventId: msg.last_event_id });
                return;
            case "snapshot": {
                let highest = this.cursor;
                for (const frame of msg.frames) {
                    highest = Math.max(highest, frame.id);
                }
                this.cursor = highest;
                this.emit({ kind: "snapshot", frames: msg.frames });
                return;
            }
            case "error":
                this.emit({ kind: "protocol-error", code: msg.code, message: msg.message });
                if (msg.code === "auth") {
                    this.stopped = true;
                    this.setStatus("disconnected");
                    this.stream?.close();
                }
                return;
            case "pong":
                return;
            case "bye":
                this.emit({ kind: "bye", reason: msg.reason });
                this.stream?.close();
                return;
        }
    }

    private acceptFrame(frame: Frame): void {
        if (frame.id <= this.cursor) {
            return;
        }
        if (this.livePhase && frame.id > this.cursor + 1) {
            this.stream?.close();
            return;
        }
        this.cursor = frame.id;
        this.emit({ kind: "frame", frame });
    }

    private async postCommand(msg: ClientMsg): Promise<CommandResult> {
        const encoded = encodeClient(msg);
        if (!encoded.ok) {
            return { ok: false, error: { kind: "out_of_bounds", field: encoded.error.kind === "out_of_bounds" ? encoded.error.field : "unknown" } };
        }
        const token = encodeURIComponent(this.options.token);
        const url = `${this.options.baseUrl}/s/${this.options.sessionId}/cmd?token=${token}`;
        const outcome = await this.options.http.postJson(url, {
            "content-type": "application/json",
            "x-harness-token": this.options.token,
        }, encoded.value);
        const status = outcome.ok ? outcome.status : (outcome.status ?? 0);
        if (status === 401 || status === 403) {
            return { ok: false, error: { kind: "auth" } };
        }
        if (status === 404) {
            return { ok: false, error: { kind: "unknown_ask" } };
        }
        if (outcome.ok) {
            return status === 202
                ? { ok: true }
                : { ok: false, error: { kind: "http", status, body: "" } };
        }
        if (outcome.networkError !== undefined) {
            return { ok: false, error: { kind: "unreachable", detail: outcome.networkError } };
        }
        return { ok: false, error: { kind: "http", status, body: "" } };
    }
}
