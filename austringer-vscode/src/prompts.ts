/**
 * Ask-prompt decisions: mapping a dialog pick to the answer the daemon
 * receives. A dismissed prompt (Esc — `undefined`) maps to `null`: the
 * ask stays PENDING until an explicit choice or the daemon's mediation
 * deadline (which resolves fail-closed on its side). Dismissal must
 * NEVER become a deny — that is the silent-deny footgun this module
 * exists to make impossible.
 *
 * The permission picks are the shared allow-scope labels (todo 23):
 * identical strings to the TUI modal and the web gate buttons.
 */

import type { Answer, ModelIdentity } from "./protocol.ts";
import { ALLOW_ONCE, ALLOW_SESSION, DENY, VOTE_CANCEL } from "./grammar.ts";

export { ALLOW_ONCE, ALLOW_SESSION, DENY, VOTE_CANCEL };

export const APPROVE_BUTTON = "Approve";
export const REJECT_BUTTON = "Reject";

export const HOP_BUTTON = (proposed: ModelIdentity): string => `Hop to ${proposed.model}`;
export const KEEP_BUTTON = (current: ModelIdentity): string => `Keep ${current.model}`;

export function isVoteCancelPick(pick: string | undefined): boolean {
    return pick === VOTE_CANCEL;
}

export function permissionAnswerFromPick(pick: string | undefined): Answer | null {
    switch (pick) {
        case ALLOW_ONCE:
            return "allow_once";
        case ALLOW_SESSION:
            return "allow_session";
        case DENY:
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
