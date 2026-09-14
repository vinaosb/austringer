/**
 * Status-bar gauge projection: pure mapping from client/turn state to
 * the one glyph + line the editor shows. The extension host applies
 * it; this module never touches the VS Code API so it stays testable
 * headless.
 */

import type { ClientStatus } from "./client.ts";

export type GaugeKind = "ok" | "warn" | "error" | "action";

export type GaugeInputs = {
    status: ClientStatus;
    attempt: number;
    turn: number | null;
    pendingAsks: number;
};

export type GaugeState = {
    text: string;
    tooltip: string;
    kind: GaugeKind;
};

export function gaugeState(inputs: GaugeInputs): GaugeState {
    if (inputs.pendingAsks > 0) {
        const plural = inputs.pendingAsks === 1 ? "ask" : "asks";
        return {
            text: `Harness: ${inputs.pendingAsks} permission ${plural} pending`,
            tooltip: "The agent is waiting on your permission decision.",
            kind: "action",
        };
    }
    switch (inputs.status) {
        case "connected":
            return inputs.turn === null
                ? { text: "Harness: connected", tooltip: "Attached to the daemon session.", kind: "ok" }
                : {
                      text: `Harness: connected — turn ${inputs.turn} running`,
                      tooltip: "A turn is in flight; steer or cancel from the omnibar.",
                      kind: "ok",
                  };
        case "connecting":
            return {
                text: `Harness: reconnecting (attempt ${inputs.attempt})`,
                tooltip: "Daemon unreachable; retrying with capped exponential backoff.",
                kind: "warn",
            };
        case "disconnected":
            return {
                text: "Harness: disconnected",
                tooltip: "Daemon refused or dropped the session. Check the daemon and reconnect.",
                kind: "error",
            };
    }
}
