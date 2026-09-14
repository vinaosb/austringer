//! Field application for the spec grammar: key/value validation and the
//! per-section applicators. Every hostile value is a typed error.

use harness_intel::elicitation::TaskClass;

use super::{Field, MAX_FIELD_BYTES, RoleDecl, SpecBudget, SpecError, TaskDecl};
use super::{has_control, split_field};
use crate::role::parse_tier;

fn parse_list(value: &str, line_no: usize, key: &str) -> Result<Vec<String>, SpecError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|item| {
            let item = item.trim();
            if item.is_empty() {
                Err(SpecError::BadValue {
                    line_no,
                    key: key.to_string(),
                    value: value.to_string(),
                })
            } else {
                Ok(item.to_string())
            }
        })
        .collect()
}

fn parse_class(value: &str) -> Option<TaskClass> {
    match value {
        "fact_find" => Some(TaskClass::FactFind),
        "comparison" => Some(TaskClass::Comparison),
        "complex" => Some(TaskClass::Complex),
        _ => None,
    }
}

pub(super) fn check_field<'a>(trimmed: &'a str, line_no: usize) -> Result<Field<'a>, SpecError> {
    let field = split_field(trimmed).ok_or(SpecError::BadHeader { line_no })?;
    if field.value.len() > MAX_FIELD_BYTES {
        return Err(SpecError::TooLarge {
            lines: line_no,
            limit: MAX_FIELD_BYTES,
        });
    }
    if has_control(field.value) {
        return Err(SpecError::BadValue {
            line_no,
            key: field.key.to_string(),
            value: "<control chars>".to_string(),
        });
    }
    Ok(Field {
        key: field.key,
        value: field.value,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_top_level(
    field: Field<'_>,
    line_no: usize,
    objectives: &mut Vec<String>,
    validators: &mut bool,
    phases: &mut bool,
) -> Result<(), SpecError> {
    match field.key {
        "objective" => {
            if field.value.is_empty() {
                return Err(SpecError::EmptyField {
                    line_no,
                    key: "objective",
                });
            }
            objectives.push(field.value.to_string());
            Ok(())
        }
        "budget" => Ok(()),
        "validators" => parse_on_off(field.value, line_no).map(|v| *validators = v),
        "phases" => parse_on_off(field.value, line_no).map(|v| *phases = v),
        other => Err(SpecError::UnknownKey {
            line_no,
            key: other.to_string(),
        }),
    }
}

fn parse_on_off(value: &str, line_no: usize) -> Result<bool, SpecError> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        other => Err(SpecError::BadBool {
            line_no,
            value: other.to_string(),
        }),
    }
}

pub(super) fn apply_budget(
    field: Field<'_>,
    line_no: usize,
    budget: &mut SpecBudget,
) -> Result<(), SpecError> {
    match field.key {
        "children" => field
            .value
            .parse::<u32>()
            .map(|v| budget.children = Some(v))
            .map_err(|_| bad_number(line_no, "children", field.value)),
        "parallel" => field
            .value
            .parse::<u32>()
            .map(|v| budget.parallel = Some(v))
            .map_err(|_| bad_number(line_no, "parallel", field.value)),
        "tokens" => field
            .value
            .parse::<u64>()
            .map(|v| budget.tokens = Some(v))
            .map_err(|_| bad_number(line_no, "tokens", field.value)),
        other => Err(SpecError::UnknownKey {
            line_no,
            key: other.to_string(),
        }),
    }
}

fn bad_number(line_no: usize, key: &str, value: &str) -> SpecError {
    SpecError::BadValue {
        line_no,
        key: key.to_string(),
        value: value.to_string(),
    }
}

pub(super) fn apply_role(
    field: Field<'_>,
    line_no: usize,
    role: &mut RoleDecl,
) -> Result<(), SpecError> {
    match field.key {
        "model" => {
            role.model = Some(field.value.to_string());
            Ok(())
        }
        "tier" => parse_tier(field.value)
            .map(|t| role.tier = Some(t))
            .ok_or_else(|| SpecError::BadValue {
                line_no,
                key: "tier".to_string(),
                value: field.value.to_string(),
            }),
        "tools" => parse_list(field.value, line_no, "tools").map(|v| role.tools = v),
        "summary" => field
            .value
            .parse::<usize>()
            .map(|v| role.summary = Some(v))
            .map_err(|_| SpecError::BadValue {
                line_no,
                key: "summary".to_string(),
                value: field.value.to_string(),
            }),
        other => Err(SpecError::UnknownKey {
            line_no,
            key: other.to_string(),
        }),
    }
}

pub(super) fn apply_task(
    field: Field<'_>,
    line_no: usize,
    task: &mut TaskDecl,
) -> Result<(), SpecError> {
    match field.key {
        "role" => {
            task.role = field.value.to_string();
            Ok(())
        }
        "objective" => {
            task.objective = field.value.to_string();
            Ok(())
        }
        "model" => {
            task.model = Some(field.value.to_string());
            Ok(())
        }
        "writes" => parse_list(field.value, line_no, "writes").map(|v| task.writes = v),
        "needs" => parse_list(field.value, line_no, "needs").map(|v| task.needs = v),
        "class" => parse_class(field.value)
            .map(|c| task.class = c)
            .ok_or_else(|| SpecError::BadValue {
                line_no,
                key: "class".to_string(),
                value: field.value.to_string(),
            }),
        other => Err(SpecError::UnknownKey {
            line_no,
            key: other.to_string(),
        }),
    }
}
