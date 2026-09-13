//! Scheduler: a minimal five-field cron engine over epoch minutes plus
//! the dispatch loop. Hand-rolled deliberately (the task allows "a
//! minimal cron expression engine or justified dep"): the `cron` crate
//! pulls chrono and wall-clock timezones, while the daemon fires on
//! injected epoch-minute boundaries — pure integer math, deterministic
//! under fake clocks (flaky-immune tests), zero new dependency weight.
//! Schedules are UTC-minute aligned; local/DST firing is a todo 41
//! concern layered on top.

use std::sync::Arc;

/// Cron parse/validation failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CronError {
    /// Wrong field count (five required).
    #[error("expected 5 fields, got {0}")]
    FieldCount(usize),
    /// A field did not parse.
    #[error("field {field:?}: {reason}")]
    Field {
        /// Field index (0=minute .. 4=dow).
        field: usize,
        /// Why it failed.
        reason: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    set: Vec<u32>,
    wildcard: bool,
}

impl Field {
    fn parse(text: &str, min: u32, max: u32, field: usize) -> Result<Self, CronError> {
        let bad = |reason: &'static str| CronError::Field { field, reason };
        let mut wildcard = false;
        let mut values: Vec<u32> = Vec::new();
        for part in text.split(',') {
            let (range_part, step) = match part.split_once('/') {
                Some((r, s)) => {
                    let step: u32 = s.parse().map_err(|_| bad("step not numeric"))?;
                    if step == 0 {
                        return Err(bad("step is zero"));
                    }
                    (r, step)
                }
                None => (part, 1),
            };
            let (lo, hi) = if range_part == "*" {
                wildcard = true;
                (min, max)
            } else if let Some((a, b)) = range_part.split_once('-') {
                let a: u32 = a.parse().map_err(|_| bad("range not numeric"))?;
                let b: u32 = b.parse().map_err(|_| bad("range not numeric"))?;
                (a, b)
            } else {
                let v: u32 = range_part.parse().map_err(|_| bad("value not numeric"))?;
                (v, v)
            };
            if lo < min || hi > max || lo > hi {
                return Err(bad("value out of range"));
            }
            let mut v = lo;
            while v <= hi {
                values.push(v);
                match v.checked_add(step) {
                    Some(next) => v = next,
                    None => break,
                }
            }
        }
        values.sort_unstable();
        values.dedup();
        Ok(Self {
            set: values,
            wildcard,
        })
    }

    fn contains(&self, v: u32) -> bool {
        self.set.contains(&v)
    }
}

/// A parsed five-field cron expression (minute hour dom month dow,
/// numeric, UTC). Standard dom/dow OR-when-both-restricted semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronExpr {
    minute: Field,
    hour: Field,
    dom: Field,
    month: Field,
    dow: Field,
}

/// Days since 1970-01-01 -> (y, m, d); Howard Hinnant's civil algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        if m <= 2 { y + 1 } else { y },
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

/// Decompose an epoch minute into calendar parts (UTC): (year, month,
/// day, hour).
fn parts(epoch_minute: u64) -> (i64, u32, u32, u32) {
    let days = (epoch_minute / 1440) as i64;
    let minute_of_day = epoch_minute % 1440;
    let (y, m, d) = civil_from_days(days);
    (y, m, d, u32::try_from(minute_of_day / 60).unwrap_or(23))
}

impl CronExpr {
    /// Parse `min hour dom month dow` (numeric fields, `*`, lists,
    /// ranges, steps; dow 0 and 7 are Sunday).
    pub fn parse(spec: &str) -> Result<Self, CronError> {
        let fields: Vec<&str> = spec.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(CronError::FieldCount(fields.len()));
        }
        let dow_field = Field::parse(fields[4].replace('7', "0").as_str(), 0, 6, 4)?;
        Ok(Self {
            minute: Field::parse(fields[0], 0, 59, 0)?,
            hour: Field::parse(fields[1], 0, 23, 1)?,
            dom: Field::parse(fields[2], 1, 31, 2)?,
            month: Field::parse(fields[3], 1, 12, 3)?,
            dow: dow_field,
        })
    }

    /// True when `epoch_minute` (UTC) matches.
    #[must_use]
    pub fn matches(&self, epoch_minute: u64) -> bool {
        let (y, m, d, h) = parts(epoch_minute);
        let minute = (epoch_minute % 60) as u32;
        if !self.minute.contains(minute) || !self.hour.contains(h) {
            return false;
        }
        if !self.month.contains(m) {
            return false;
        }
        let days = (epoch_minute / 1440) as i64;
        let dow = ((days + 4).rem_euclid(7)) as u32;
        let dom_ok = self.dom.contains(d);
        let dow_ok = self.dow.contains(dow);
        let year_ok = y >= 1970;
        year_ok
            && if !self.dom.wildcard && !self.dow.wildcard {
                dom_ok || dow_ok
            } else {
                dom_ok && dow_ok
            }
    }

    /// First matching minute strictly after `epoch_minute`, scanning at
    /// most 366 days ahead (None = effectively never).
    #[must_use]
    pub fn next_after(&self, epoch_minute: u64) -> Option<u64> {
        let horizon = epoch_minute.saturating_add(366 * 1440);
        let mut candidate = epoch_minute.saturating_add(1);
        while candidate <= horizon {
            if self.matches(candidate) {
                return Some(candidate);
            }
            candidate = candidate.saturating_add(1);
        }
        None
    }
}

/// What a fired job does (todo 41 grows the pack vocabulary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobAction {
    /// Deliver a typed notification.
    Notify {
        /// Notification kind.
        kind: String,
        /// Message text.
        message: String,
    },
    /// Enqueue a scheduled pack run.
    RunPack {
        /// Pack name.
        pack: String,
    },
}

/// One scheduled job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledJob {
    /// Stable job id.
    pub id: String,
    /// Human label.
    pub name: String,
    /// When to fire.
    pub cron: CronExpr,
    /// What to do.
    pub action: JobAction,
}

/// A job firing produced by [`Scheduler::tick`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fired {
    /// Job that fired.
    pub job_id: String,
    /// The matching epoch minute.
    pub at_minute: u64,
    /// The action to dispatch.
    pub action: JobAction,
}

/// How many missed minutes catch up in one tick before coalescing
/// (jittered catch-up is todo 41; the bound keeps tick bounded).
pub const MAX_CATCHUP_MINUTES: u64 = 60 * 24;

/// Clock seam: epoch seconds.
pub type SchedulerClock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// The dispatcher. Deterministic: firing is a pure function of the
/// injected clock, so tests (and flake-runs) are reproducible.
pub struct Scheduler {
    jobs: Vec<ScheduledJob>,
    last_minute: Option<u64>,
}

impl Scheduler {
    /// New scheduler over `jobs`, starting from "never ticked".
    #[must_use]
    pub fn new(jobs: Vec<ScheduledJob>) -> Self {
        Self {
            jobs,
            last_minute: None,
        }
    }

    /// Advance to `now_epoch_secs`, firing each job at most once per
    /// matched minute. The first tick only establishes the baseline
    /// (no catch-up fire from process start).
    pub fn tick(&mut self, now_epoch_secs: u64) -> Vec<Fired> {
        let now_minute = now_epoch_secs / 60;
        let mut fired = Vec::new();
        let Some(last) = self.last_minute else {
            self.last_minute = Some(now_minute);
            return fired;
        };
        let end = now_minute.min(last.saturating_add(MAX_CATCHUP_MINUTES));
        let mut m = last.saturating_add(1);
        while m <= end {
            for job in &self.jobs {
                if job.cron.matches(m) {
                    fired.push(Fired {
                        job_id: job.id.clone(),
                        at_minute: m,
                        action: job.action.clone(),
                    });
                }
            }
            m = m.saturating_add(1);
        }
        self.last_minute = Some(now_minute);
        fired
    }

    /// Jobs under management.
    #[must_use]
    pub fn jobs(&self) -> &[ScheduledJob] {
        &self.jobs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
        let y_adj = if m <= 2 { y - 1 } else { y };
        let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
        let yoe = y_adj - era * 400;
        let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
        let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    fn minute_of(y: i64, m: u32, d: u32, h: u32, min: u32) -> u64 {
        u64::try_from(days_from_civil(y, m, d)).unwrap_or(0) * 1440
            + u64::from(h) * 60
            + u64::from(min)
    }

    #[test]
    fn calendar_decomposition_anchors() -> Result<(), CronError> {
        // 1970-01-01 was a Thursday (dow 4).
        assert_eq!(parts(0), (1970, 1, 1, 0));
        // 2026-09-13 is a Sunday.
        let m = minute_of(2026, 9, 13, 0, 0);
        assert_eq!(parts(m), (2026, 9, 13, 0));
        let days = i64::try_from(m / 1440).unwrap_or(0);
        assert_eq!((days + 4).rem_euclid(7), 0);
        Ok(())
    }

    #[test]
    fn every_five_minutes_fires_on_multiples() -> Result<(), CronError> {
        let cron = CronExpr::parse("*/5 * * * *")?;
        assert!(cron.matches(0));
        assert!(!cron.matches(3));
        assert!(cron.matches(300));
        assert_eq!(cron.next_after(1), Some(5));
        Ok(())
    }

    #[test]
    fn weekday_nine_utc() -> Result<(), CronError> {
        let monday_0900 = minute_of(2026, 9, 14, 9, 0);
        let sunday_0900 = minute_of(2026, 9, 13, 9, 0);
        let cron = CronExpr::parse("0 9 * * mon-fri");
        assert_eq!(
            cron,
            Err(CronError::Field {
                field: 4,
                reason: "range not numeric"
            })
        );
        let cron = CronExpr::parse("0 9 * * 1-5")?;
        assert!(cron.matches(monday_0900));
        assert!(!cron.matches(sunday_0900));
        assert!(!cron.matches(monday_0900 + 60 * 6));
        Ok(())
    }

    #[test]
    fn lists_ranges_and_steps_parse() -> Result<(), CronError> {
        assert!(CronExpr::parse("1,15 9-17 */2 1,6 0").is_ok());
        assert_eq!(CronExpr::parse("* * * *"), Err(CronError::FieldCount(4)));
        assert_eq!(
            CronExpr::parse("99 * * * *"),
            Err(CronError::Field {
                field: 0,
                reason: "value out of range"
            })
        );
        assert_eq!(
            CronExpr::parse("* * 0 * *"),
            Err(CronError::Field {
                field: 2,
                reason: "value out of range"
            })
        );
        Ok(())
    }

    #[test]
    fn dow_seven_is_sunday_zero() -> Result<(), CronError> {
        let a = CronExpr::parse("* * * * 7")?;
        let b = CronExpr::parse("* * * * 0")?;
        let sunday = minute_of(2026, 9, 13, 12, 0);
        assert!(a.matches(sunday));
        assert!(b.matches(sunday));
        Ok(())
    }

    #[test]
    fn dom_dow_or_when_both_restricted() -> Result<(), CronError> {
        let c = CronExpr::parse("0 0 13 * 0")?;
        let sep13_sunday = minute_of(2026, 9, 13, 0, 0);
        let sep20_sunday = minute_of(2026, 9, 20, 0, 0);
        let oct13_tuesday = minute_of(2026, 10, 13, 0, 0);
        assert!(c.matches(sep13_sunday));
        assert!(c.matches(sep20_sunday));
        assert!(c.matches(oct13_tuesday));
        assert!(!c.matches(minute_of(2026, 9, 14, 0, 0)));
        Ok(())
    }

    #[test]
    fn next_after_wraps_month_and_year() -> Result<(), CronError> {
        let cron = CronExpr::parse("0 0 1 1 *")?;
        let after_dec_2026 = minute_of(2026, 12, 25, 0, 0);
        let expected = minute_of(2027, 1, 1, 0, 0);
        assert_eq!(cron.next_after(after_dec_2026), Some(expected));
        Ok(())
    }

    #[test]
    fn scheduler_fires_deterministically_on_injected_clock() -> Result<(), CronError> {
        let job = ScheduledJob {
            id: "nightly".into(),
            name: "nightly pack".into(),
            cron: CronExpr::parse("*/5 * * * *")?,
            action: JobAction::RunPack {
                pack: "review".into(),
            },
        };
        let mut sched = Scheduler::new(vec![job]);
        assert!(sched.tick(0).is_empty());
        let fired = sched.tick(600);
        assert_eq!(fired.len(), 2);
        assert_eq!(fired[0].at_minute, 5);
        assert_eq!(fired[1].at_minute, 10);
        assert!(sched.tick(600).is_empty());
        assert!(sched.tick(780).is_empty());
        Ok(())
    }

    #[test]
    fn scheduler_catchup_is_bounded() -> Result<(), CronError> {
        let job = ScheduledJob {
            id: "j".into(),
            name: "j".into(),
            cron: CronExpr::parse("* * * * *")?,
            action: JobAction::Notify {
                kind: "k".into(),
                message: "m".into(),
            },
        };
        let mut sched = Scheduler::new(vec![job]);
        sched.tick(0);
        let fired = sched.tick(7 * 86400);
        assert_eq!(fired.len() as u64, MAX_CATCHUP_MINUTES);
        Ok(())
    }
}
