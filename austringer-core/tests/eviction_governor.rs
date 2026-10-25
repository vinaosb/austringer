//! Two-tier per-class call governors (plan todo 12).
//!
//! Under test: per-class budgets keyed on a TaskClass classifier input
//! (never one global cap — the shipped classes span ~10x); the SOFT budget
//! surfaces a typed notice injected into context; the HARD budget DENIES
//! further calls with a structured error the model reads; the classifier
//! is a seam (keyword heuristic now, real classifier with profiles later).

mod common;

use austringer_core::eviction::{
    CallGovernor, GovernorConfig, GovernorVerdict, KeywordClassifier, TaskClass, TaskClassifier,
};

#[test]
fn shipped_class_budgets_span_a_ten_x_spread_and_key_per_class() {
    let config = GovernorConfig::default();
    let light = config
        .budget_for(TaskClass::LightChat)
        .expect("light chat budgeted");
    let research = config
        .budget_for(TaskClass::DeepResearch)
        .expect("deep research budgeted");
    assert!(
        research.soft >= light.soft * 10,
        "documented ~10x spread: research.soft={} light.soft={}",
        research.soft,
        light.soft
    );
    for class in TaskClass::ALL {
        let budget = config
            .budget_for(class)
            .unwrap_or_else(|| panic!("class {class:?} must carry its own budget"));
        assert!(budget.hard > budget.soft, "hard must exceed soft");
    }
    // There is no global fallback: an unbudgeted class is a typed miss.
    let mut partial = GovernorConfig::default();
    partial.per_class.clear();
    assert!(partial.budget_for(TaskClass::CodeEdit).is_none());
}

#[test]
fn keyword_classifier_seam_routes_prompts_to_task_classes() {
    let classifier = KeywordClassifier;
    assert_eq!(
        classifier.classify("hey, what is a borrow checker?"),
        TaskClass::LightChat
    );
    assert_eq!(
        classifier.classify("edit the file and fix the failing test"),
        TaskClass::CodeEdit
    );
    assert_eq!(
        classifier.classify("research all crates and compare benchmarks"),
        TaskClass::DeepResearch
    );
    assert_eq!(classifier.classify(""), TaskClass::LightChat);
}

#[test]
fn soft_budget_crossing_surfaces_a_typed_notice_for_context_injection() {
    let config = GovernorConfig::default();
    let soft = config
        .budget_for(TaskClass::LightChat)
        .map(|budget| budget.soft)
        .expect("light chat budgeted");
    let mut governor = CallGovernor::new(config, TaskClass::LightChat);
    for _ in 0..soft.saturating_sub(1) {
        governor.record_call();
    }
    assert!(matches!(
        governor.check("view_window"),
        GovernorVerdict::Allow
    ));
    governor.record_call();
    match governor.check("view_window") {
        GovernorVerdict::AllowWithNotice(notice) => {
            let text = notice.render();
            assert!(text.starts_with("<call_budget_notice "));
            assert!(text.contains("class=\"light_chat\""));
            assert!(text.contains(&format!("used=\"{soft}\"")));
            assert!(text.contains("strategy=\"conserve-calls\""));
        }
        other => panic!("at soft must notice, got {other:?}"),
    }
}

#[test]
fn hard_budget_denies_further_calls_with_a_structured_error_the_model_reads() {
    let config = GovernorConfig::default();
    let hard = config
        .budget_for(TaskClass::LightChat)
        .map(|budget| budget.hard)
        .expect("light chat budgeted");
    let mut governor = CallGovernor::new(config, TaskClass::LightChat);
    for _ in 0..hard {
        governor.record_call();
    }
    match governor.check("execute_command") {
        GovernorVerdict::Denied(denial) => {
            let text = denial.render();
            assert!(text.starts_with("<call_denied "));
            assert!(text.ends_with("</call_denied>"));
            assert!(text.contains("tool=\"execute_command\""));
            assert!(text.contains(&format!("used=\"{hard}\"")));
            assert!(text.contains(&format!("hard=\"{hard}\"")));
            assert!(
                text.contains("stop calling tools"),
                "the denial must tell the model what to do next"
            );
        }
        other => panic!("at hard must deny, got {other:?}"),
    }
}

#[test]
fn budgets_are_per_class_not_one_global_cap() {
    // The same call count exhausts LightChat but stays within DeepResearch.
    let config = GovernorConfig::default();
    let light_hard = config
        .budget_for(TaskClass::LightChat)
        .map(|budget| budget.hard)
        .expect("light chat budgeted");
    let mut research = CallGovernor::new(config.clone(), TaskClass::DeepResearch);
    for _ in 0..light_hard {
        research.record_call();
    }
    assert!(matches!(
        research.check("web_search"),
        GovernorVerdict::Allow | GovernorVerdict::AllowWithNotice(_)
    ));
    let mut light = CallGovernor::new(config, TaskClass::LightChat);
    for _ in 0..light_hard {
        light.record_call();
    }
    assert!(matches!(
        light.check("web_search"),
        GovernorVerdict::Denied(_)
    ));
}
