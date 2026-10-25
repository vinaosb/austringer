import { test } from "node:test";
import assert from "node:assert/strict";

import {
    permissionAnswerFromPick,
    fallbackChoiceFromPick,
    planDecisionFromPick,
    isVoteCancelPick,
    HOP_BUTTON,
    KEEP_BUTTON,
    APPROVE_BUTTON,
    REJECT_BUTTON,
    ALLOW_ONCE,
    ALLOW_SESSION,
    DENY,
    VOTE_CANCEL,
} from "../src/prompts.ts";

test("Esc (dismissed prompt) leaves the permission ask pending — never denies", () => {
    const answer = permissionAnswerFromPick(undefined);
    assert.equal(answer, null, "dismissal must map to null (stay pending), not an answer");
});

test("explicit permission picks map to their answers (todo 23 canonical labels)", () => {
    assert.equal(permissionAnswerFromPick("y (once)"), "allow_once");
    assert.equal(permissionAnswerFromPick("a (session)"), "allow_session");
    assert.equal(permissionAnswerFromPick("n (deny)"), "deny");
    assert.equal(ALLOW_ONCE, "y (once)");
    assert.equal(ALLOW_SESSION, "a (session)");
    assert.equal(DENY, "n (deny)");
    assert.equal(VOTE_CANCEL, "c (vote-cancel)");
});

test("vote-cancel is a withdraw pick, never an answer", () => {
    assert.equal(isVoteCancelPick("c (vote-cancel)"), true);
    assert.equal(isVoteCancelPick(undefined), false);
    assert.equal(permissionAnswerFromPick("c (vote-cancel)"), null, "withdraw must not resolve as an answer");
});

test("deny is only reachable through the explicit deny pick", () => {
    const dismissed = permissionAnswerFromPick(undefined);
    assert.notEqual(dismissed, "deny");
    assert.notEqual(dismissed, "allow_once");
    assert.notEqual(dismissed, "allow_session");
});

test("fallback ask: Esc stays pending; explicit picks choose hop or keep", () => {
    const current = { provider: "anthropic", model: "claude-haiku-5.5" };
    const proposed = { provider: "openai", model: "gpt-6.1-sol" };
    assert.equal(fallbackChoiceFromPick(undefined, current, proposed), null);
    assert.equal(fallbackChoiceFromPick("Hop to gpt-6.1-sol", current, proposed), proposed);
    assert.equal(fallbackChoiceFromPick("Keep claude-haiku-5.5", current, proposed), current);
});

test("fallback buttons are derived from the ask identities so picks are unambiguous", () => {
    const current = { provider: "anthropic", model: "claude-haiku-5.5" };
    const proposed = { provider: "openai", model: "gpt-6.1-sol" };
    assert.equal(HOP_BUTTON(proposed), "Hop to gpt-6.1-sol");
    assert.equal(KEEP_BUTTON(current), "Keep claude-haiku-5.5");
});

test("plan ask: Esc stays pending; explicit picks approve or reject", () => {
    assert.equal(planDecisionFromPick(undefined), null);
    assert.equal(planDecisionFromPick("Approve"), "approve");
    assert.equal(planDecisionFromPick("Reject"), "reject");
    assert.equal(APPROVE_BUTTON, "Approve");
    assert.equal(REJECT_BUTTON, "Reject");
});
