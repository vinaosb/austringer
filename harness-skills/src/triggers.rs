//! Trigger derivation from description prose: the `USE FOR:` /
//! `DO NOT USE FOR:` convention used across the real skill corpus, plus
//! the always-present `/name` command trigger.

use crate::skill::SkillMeta;

/// Fills derived trigger phrases when none were declared explicitly:
/// `USE FOR:` / `DO NOT USE FOR:` segments of the description, plus the
/// `/name` command. Derived negative scoping only applies when no
/// explicit `do-not-trigger-on` was declared.
pub(crate) fn derive_triggers(meta: &mut SkillMeta) {
    let slash = format!("/{}", meta.name.as_str());
    if !meta.triggers.is_empty() {
        meta.triggers.push(slash);
        return;
    }
    let (use_for, do_not) = split_use_for_segments(&meta.description);
    meta.triggers = use_for;
    meta.triggers.push(slash);
    if meta.negative_triggers.is_empty() {
        meta.negative_triggers = do_not;
    }
}

fn split_use_for_segments(description: &str) -> (Vec<String>, Vec<String>) {
    let Some((_, rest)) = description.split_once("USE FOR:") else {
        return (Vec::new(), Vec::new());
    };
    let (use_seg, do_not_seg) = match rest.split_once("DO NOT USE FOR:") {
        Some((u, d)) => (u, Some(d)),
        None => (rest, None),
    };
    let split_items = |seg: &str| {
        seg.trim()
            .trim_end_matches('.')
            .split(", ")
            .map(|i| i.trim().trim_end_matches('.').to_string())
            .filter(|i| !i.is_empty())
            .collect::<Vec<_>>()
    };
    (
        split_items(use_seg),
        do_not_seg.map(split_items).unwrap_or_default(),
    )
}
