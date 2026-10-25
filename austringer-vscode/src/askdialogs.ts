/**
 * Ask-dialog glue: the warning-message surfaces for permission,
 * model-fallback, and plan-approval asks. Every dialog maps a
 * DISMISSED prompt (Esc, `undefined`) to "leave the ask pending" —
 * the daemon's mediation deadline resolves it fail-closed; this
 * client never turns a dismissal into a deny.
 */

import * as vscode from "vscode";

import type { HarnessClient } from "./client.ts";
import type { Answer, AskPayload, FallbackAskPayload, PlanAskPayload } from "./protocol.ts";
import {
    fallbackChoiceFromPick,
    isVoteCancelPick,
    KEEP_BUTTON,
    HOP_BUTTON,
    permissionAnswerFromPick,
    planDecisionFromPick,
    APPROVE_BUTTON,
    REJECT_BUTTON,
    ALLOW_ONCE,
    ALLOW_SESSION,
    DENY,
    VOTE_CANCEL,
} from "./prompts.ts";

export class AskDialogs {
    constructor(private readonly client: HarnessClient) {}

    async answer(askId: number, answer: Answer): Promise<void> {
        const result = await this.client.answerAsk(askId, answer);
        if (!result.ok && result.error.kind !== "unknown_ask") {
            void vscode.window.showErrorMessage(`Austringer: answer failed (${result.error.kind})`);
        }
    }

    async promptPermission(ask: AskPayload): Promise<void> {
        const pick = await vscode.window.showWarningMessage(
            `${ask.tool} → ${ask.target ?? "?"}: ${ask.reason}`,
            ALLOW_ONCE,
            ALLOW_SESSION,
            DENY,
            VOTE_CANCEL,
        );
        if (isVoteCancelPick(pick)) {
            const result = await this.client.cancelAsk(ask.ask_id);
            if (!result.ok && result.error.kind !== "unknown_ask") {
                void vscode.window.showErrorMessage(`Austringer: vote-cancel failed (${result.error.kind})`);
            }
            return;
        }
        const answer = permissionAnswerFromPick(pick);
        if (answer === null) {
            return;
        }
        await this.answer(ask.ask_id, answer);
    }

    async promptFallback(ask: FallbackAskPayload): Promise<void> {
        const pick = await vscode.window.showWarningMessage(
            `Model fallback asked: ${ask.current.model} → ${ask.proposed.model} (${ask.reason}). ` +
                "No answer keeps the current model when the deadline fires.",
            HOP_BUTTON(ask.proposed),
            KEEP_BUTTON(ask.current),
        );
        const chosen = fallbackChoiceFromPick(pick, ask.current, ask.proposed);
        if (chosen === null) {
            return;
        }
        const result = await this.client.answerFallback(ask.ask_id, chosen);
        if (!result.ok) {
            void vscode.window.showErrorMessage(`Austringer: fallback answer failed (${result.error.kind})`);
        }
    }

    async promptPlan(ask: PlanAskPayload): Promise<void> {
        const pick = await vscode.window.showWarningMessage(
            `Plan ${ask.plan_id} (revision ${ask.revision}) awaits approval — writes stay gated until approved. ` +
                "See the artifact in the Austringer transcript.",
            APPROVE_BUTTON,
            REJECT_BUTTON,
        );
        const decision = planDecisionFromPick(pick);
        if (decision === null) {
            return;
        }
        const result = await this.client.answerPlan(ask.ask_id, decision);
        if (!result.ok) {
            void vscode.window.showErrorMessage(`Austringer: plan answer failed (${result.error.kind})`);
        }
    }
}
