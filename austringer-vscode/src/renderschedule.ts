/**
 * Render coalescing policy for the transcript webview: appending an
 * entry per daemon frame must NOT reassign `webview.html` per event —
 * that assignment destroys and recreates the whole webview iframe and
 * resets scroll. This module decides WHEN the panel may re-render:
 * trailing-edge coalescing (one render per window) plus visibility
 * gating (hidden panels stay dirty; reveal flushes once). Pure and
 * clock-injected so tests need no real waits; the panel glue owns the
 * actual timer.
 */

export type RenderScheduler = {
    /**
     * Record activity at instant `now`. Returns the absolute instant a
     * flush is due (the glue should arm its timer for it), or null when
     * the push only marked the buffer dirty (window already armed, or
     * panel hidden — reveal will flush).
     */
    push: (now: number) => number | null;
    /** Consume the armed flush at instant `now`; true means "render now". */
    takeFlush: (now: number) => boolean;
    /**
     * Track panel visibility. Hiding never renders; revealing with a
     * dirty buffer returns true exactly once — render immediately.
     */
    setVisible: (visible: boolean, now: number) => boolean;
    /** Clear dirty state after a render that bypassed the scheduler (panel creation). */
    markRendered: () => void;
};

export function createRenderScheduler(options: { windowMs: number }): RenderScheduler {
    let visible = true;
    let dirty = false;
    let dueAt: number | null = null;

    return {
        push(now) {
            dirty = true;
            if (!visible || dueAt !== null) {
                return null;
            }
            dueAt = now + options.windowMs;
            return dueAt;
        },
        takeFlush(now) {
            if (dueAt === null || now < dueAt) {
                return false;
            }
            dueAt = null;
            if (!visible) {
                return false;
            }
            dirty = false;
            return true;
        },
        setVisible(nowVisible) {
            const wasVisible = visible;
            visible = nowVisible;
            // A pending window is meaningless across a visibility change:
            // reveal renders dirty state immediately or nothing at all.
            dueAt = null;
            if (nowVisible && dirty && !wasVisible) {
                dirty = false;
                return true;
            }
            return false;
        },
        markRendered() {
            dirty = false;
            dueAt = null;
        },
    };
}

/** Append keeping `entries` capped at `cap` by dropping the oldest. */
export function appendBounded<T>(entries: T[], entry: T, cap: number): void {
    entries.push(entry);
    if (entries.length > cap) {
        entries.splice(0, entries.length - cap);
    }
}

export const MAX_ENTRIES = 500;
