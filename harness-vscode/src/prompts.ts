/**
 * Ask-prompt decisions: mapping a dialog pick to the answer the daemon
 * receives. A dismissed prompt (Esc — `undefined`) maps to `null`: the
 * ask stays PENDING until an explicit choice or the daemon's mediation
 * deadline (which resolves fail-closed on its side). Dismissal must
 * NEVER become a deny — that is the silent-deny footgun this module
 * exists to make impossible.
 */

import type { Answer, ModelIdentity } from "./protocol.ts";

export const APPROVE_BUTTON = "Approve";
export const REJECT_BUTTON = "Reject";

export const HOP_BUTTON = (proposed: ModelIdentity): string => `Hop to ${proposed.model}`;
export const KEEP_BUTTON = (current: ModelIdentity): string => `Keep ${current.model}`;

export function permissionAnswerFromPick(pick: string | undefined): Answer | null {
    switch (pick) {
        case "Allow once":
            return "allow_once";
        case "Allow for session":
            return "allow_session";
        case "Deny":
            return "deny";
        default:
            return null;
    }
}

export function fallbackChoiceFromPick(
    pick: string | undefined,
    current: ModelIdentity,
    proposed: ModelIdentity,
): ModelIdentity | null {
    if (pick === HOP_BUTTON(proposed)) {
        return proposed;
    }
    if (pick === KEEP_BUTTON(current)) {
        return current;
    }
    return null;
}

export function planDecisionFromPick(pick: string | undefined): "approve" | "reject" | null {
    switch (pick) {
        case APPROVE_BUTTON:
            return "approve";
        case REJECT_BUTTON:
            return "reject";
        default:
            return null;
    }
}
