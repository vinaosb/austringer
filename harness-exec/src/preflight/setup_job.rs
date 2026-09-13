//! The semantic layer of the setup-steps parser (plan todo 22): the
//! branches/job/steps block parsers over the [`Lines`] cursor from
//! [`super::setup_parse`].

use super::setup::{SetupJob, SetupStep, SetupStepsError};
use super::setup_parse::{Lines, split_flow_list, split_key, strip_quotes};

pub(super) fn parse_branches(cursor: &mut Lines<'_>) -> Result<String, SetupStepsError> {
    let branches_line = match cursor.next() {
        Some(line) if line.indent == 4 && split_key(line.content).0 == "branches" => line,
        Some(line) if line.indent == 4 => {
            return Err(SetupStepsError::UnknownKey {
                key: split_key(line.content).0.to_owned(),
                line: line.number,
            });
        }
        // Dedent back to the top level (e.g. straight to `jobs:`) means
        // the branches block was never written.
        Some(line) if line.indent < 4 => {
            return Err(SetupStepsError::MissingField {
                field: "branches",
                line: line.number,
            });
        }
        Some(line) => {
            return Err(SetupStepsError::Unexpected {
                line: line.number,
                text: line.content.to_owned(),
            });
        }
        None => {
            return Err(SetupStepsError::MissingField {
                field: "branches",
                line: 1,
            });
        }
    };
    let (_, value) = split_key(branches_line.content);
    let entries = if value.is_empty() {
        let mut collected = Vec::new();
        while let Some(line) = cursor.peek()
            && line.indent == 6
            && let Some(item) = line.content.strip_prefix("- ")
        {
            collected.push((line.number, strip_quotes(item.trim())));
            cursor.at += 1;
        }
        if collected.is_empty() {
            return Err(SetupStepsError::MissingField {
                field: "branches",
                line: branches_line.number,
            });
        }
        collected
    } else {
        split_flow_list(value, branches_line.number, "branches")?
            .into_iter()
            .map(|entry| (branches_line.number, entry))
            .collect::<Vec<_>>()
    };
    match entries.as_slice() {
        [(_, branch)] => Ok(branch.clone()),
        [] => Err(SetupStepsError::MissingField {
            field: "branches",
            line: branches_line.number,
        }),
        _ => Err(SetupStepsError::MultipleBranches {
            line: branches_line.number,
        }),
    }
}

pub(super) fn parse_job(cursor: &mut Lines<'_>) -> Result<SetupJob, SetupStepsError> {
    let job_line = match cursor.next() {
        Some(line) if line.indent == 2 && line.content.ends_with(':') => line,
        Some(line) if line.indent == 2 => {
            return Err(SetupStepsError::Unexpected {
                line: line.number,
                text: line.content.to_owned(),
            });
        }
        Some(line) => {
            return Err(SetupStepsError::UnknownKey {
                key: split_key(line.content).0.to_owned(),
                line: line.number,
            });
        }
        None => {
            return Err(SetupStepsError::MissingField {
                field: "jobs entry",
                line: 1,
            });
        }
    };
    let name = strip_quotes(split_key(job_line.content).0);
    let mut runs_on: Option<String> = None;
    let mut steps: Option<Vec<SetupStep>> = None;
    while let Some(line) = cursor.peek() {
        if line.indent != 4 {
            break;
        }
        let (key, value) = split_key(line.content);
        let number = line.number;
        cursor.at += 1;
        match key {
            "runs-on" => {
                if value.is_empty() {
                    return Err(SetupStepsError::MissingField {
                        field: "runs-on",
                        line: number,
                    });
                }
                runs_on = Some(strip_quotes(value));
            }
            "steps" => steps = Some(parse_steps(cursor, number)?),
            other => {
                return Err(SetupStepsError::UnknownKey {
                    key: other.to_owned(),
                    line: number,
                });
            }
        }
    }
    match steps {
        Some(steps) if !steps.is_empty() => Ok(SetupJob {
            name,
            runs_on,
            steps,
        }),
        Some(_) | None => Err(SetupStepsError::EmptySteps {
            line: job_line.number,
        }),
    }
}

fn parse_steps(
    cursor: &mut Lines<'_>,
    steps_line: usize,
) -> Result<Vec<SetupStep>, SetupStepsError> {
    let mut steps = Vec::new();
    while let Some(line) = cursor.peek() {
        if line.indent != 6 {
            break;
        }
        let number = line.number;
        let Some(body) = line.content.strip_prefix("- ") else {
            return Err(SetupStepsError::Unexpected {
                line: number,
                text: line.content.to_owned(),
            });
        };
        cursor.at += 1;
        let (key, value) = split_key(body);
        match key {
            "run" if !value.is_empty() => steps.push(SetupStep {
                name: None,
                run: strip_quotes(value),
            }),
            "name" => steps.push(parse_named_step(cursor, number, value)?),
            "run" => {
                return Err(SetupStepsError::MissingField {
                    field: "run",
                    line: number,
                });
            }
            other => {
                return Err(SetupStepsError::UnknownKey {
                    key: other.to_owned(),
                    line: number,
                });
            }
        }
    }
    if steps.is_empty() {
        return Err(SetupStepsError::EmptySteps { line: steps_line });
    }
    Ok(steps)
}

/// A `- name: <label>` item: the `run:` continuation sits at indent 8.
fn parse_named_step(
    cursor: &mut Lines<'_>,
    item_line: usize,
    value: &str,
) -> Result<SetupStep, SetupStepsError> {
    let name = Some(strip_quotes(value));
    let mut run: Option<String> = None;
    while let Some(inner) = cursor.peek() {
        if inner.indent < 8 {
            break;
        }
        let (inner_key, inner_value) = split_key(inner.content);
        let inner_number = inner.number;
        cursor.at += 1;
        match inner_key {
            "run" if !inner_value.is_empty() => run = Some(strip_quotes(inner_value)),
            other => {
                return Err(SetupStepsError::UnknownKey {
                    key: other.to_owned(),
                    line: inner_number,
                });
            }
        }
    }
    let Some(run) = run else {
        return Err(SetupStepsError::MissingField {
            field: "run",
            line: item_line,
        });
    };
    Ok(SetupStep { name, run })
}
