use crate::config::{Config, CountdownSound, EventConfig};
use chrono::{Datelike, Local, NaiveTime, Timelike};

#[derive(Debug, Clone)]
pub enum DisplayState {
    /// No upcoming event in countdown range.
    Idle,
    /// Counting down: event name, seconds remaining, whether to highlight.
    Countdown {
        name: String,
        seconds_left: u64,
        highlight: bool,
        sound: String,
        /// Sounds that should fire at this exact second.
        sounds_to_play: Vec<CountdownSound>,
    },
    /// Event is live right now.
    Live { name: String, sound: String },
}

impl DisplayState {
    pub fn menu_bar_text(&self) -> String {
        match self {
            DisplayState::Idle => String::new(),
            DisplayState::Countdown {
                name, seconds_left, ..
            } => {
                let mins = seconds_left / 60;
                let secs = seconds_left % 60;
                format!("((\u{2022})) {name} in {mins}:{secs:02}")
            }
            DisplayState::Live { name, .. } => {
                format!("((\u{2022})) {name} is live!")
            }
        }
    }

    pub fn should_highlight(&self) -> bool {
        matches!(
            self,
            DisplayState::Countdown {
                highlight: true,
                ..
            } | DisplayState::Live { .. }
        )
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, DisplayState::Idle)
    }
}

/// Evaluate all events and return the most relevant display state.
pub fn evaluate(config: &Config) -> DisplayState {
    let now = Local::now();
    let today = now.date_naive();
    let current_time = now.time();
    let weekday = now.weekday();

    let mut best_live: Option<(DisplayState, u64)> = None;
    let mut best_countdown: Option<(DisplayState, u64)> = None;

    for event in &config.events {
        if !event_active_today(event, today, weekday) {
            continue;
        }

        let Some(event_time) = event.parsed_time() else {
            continue;
        };

        let elapsed = elapsed_since(current_time, event_time);
        let remaining = seconds_until_time(current_time, event_time);

        // "Live" state: event_time has passed and we're within live_duration
        if elapsed <= config.live_duration_secs
            && remaining > (86400 - config.live_duration_secs - 1)
        {
            // elapsed is small (just passed) — we're in the live window
            if best_live.as_ref().is_none_or(|(_, e)| elapsed < *e) {
                best_live = Some((
                    DisplayState::Live {
                        name: event.name.clone(),
                        sound: event.sound.clone(),
                    },
                    elapsed,
                ));
            }
            continue;
        }

        // "Countdown" state: event is upcoming within countdown_start_secs
        if remaining > 0 && remaining <= event.countdown_start_secs {
            let highlight = event.highlight && remaining <= event.highlight_at_secs;

            // Collect sounds that should fire at this exact second
            let sounds_to_play: Vec<CountdownSound> = event
                .countdown_sounds
                .iter()
                .filter(|s| s.at_secs == remaining)
                .cloned()
                .collect();

            if best_countdown.as_ref().is_none_or(|(_, r)| remaining < *r) {
                best_countdown = Some((
                    DisplayState::Countdown {
                        name: event.name.clone(),
                        seconds_left: remaining,
                        highlight,
                        sound: event.sound.clone(),
                        sounds_to_play,
                    },
                    remaining,
                ));
            }
        }
    }

    // Live takes priority over countdown
    best_live
        .or(best_countdown)
        .map(|(state, _)| state)
        .unwrap_or(DisplayState::Idle)
}

/// Returns the number of seconds until the event's next scheduled occurrence,
/// looking ahead at most 7 days (to handle weekly events). Returns `None` if
/// the event has no valid parsed time or no active day within the search window.
pub fn seconds_until_next_occurrence(event: &EventConfig, config: &Config) -> Option<u64> {
    let now = Local::now();
    let current_time = now.time();
    let today = now.date_naive();

    let event_time = event.parsed_time()?;

    // Check today first, then up to 6 days ahead (covers weekly schedules).
    for days_ahead in 0u64..=6 {
        let candidate_date = today + chrono::Duration::days(days_ahead as i64);
        let weekday = candidate_date.weekday();

        if !event_active_today(event, candidate_date, weekday) {
            continue;
        }

        let secs_in_day = if days_ahead == 0 {
            seconds_until_time(current_time, event_time)
        } else {
            // Midnight boundary: seconds until end of today + seconds into candidate day
            let seconds_remaining_today =
                seconds_until_time(current_time, NaiveTime::from_hms_opt(0, 0, 0).unwrap());
            let seconds_into_day = time_to_secs(event_time);
            seconds_remaining_today + seconds_into_day
        };

        // Skip if the event is currently in its live window (it has already started today)
        if days_ahead == 0 {
            let elapsed = elapsed_since(current_time, event_time);
            if elapsed <= config.live_duration_secs
                && secs_in_day > (86400 - config.live_duration_secs - 1)
            {
                // Currently live — report 0 remaining
                return Some(0);
            }
        }

        return Some(secs_in_day);
    }

    None
}

/// Format a seconds-until value as a compact human-readable string.
/// Examples: "live!", "0:45", "12:34", "1h 23m", "2d 4h"
pub fn format_time_until(secs: u64) -> String {
    if secs == 0 {
        return "live!".into();
    }
    if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        return format!("{m}:{s:02}");
    }
    if secs < 86400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        return format!("{h}h {m}m");
    }
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    format!("{d}d {h}h")
}

fn event_active_today(
    event: &EventConfig,
    today: chrono::NaiveDate,
    weekday: chrono::Weekday,
) -> bool {
    let dates = event.parsed_dates();
    if !dates.is_empty() {
        return dates.contains(&today);
    }

    let weekdays = event.parsed_weekdays();
    if !weekdays.is_empty() {
        return weekdays.contains(&weekday);
    }

    // No constraints = every day
    true
}

/// Seconds from `current` until `target`, wrapping at midnight.
fn seconds_until_time(current: NaiveTime, target: NaiveTime) -> u64 {
    let current_secs = time_to_secs(current);
    let target_secs = time_to_secs(target);

    if target_secs > current_secs {
        target_secs - current_secs
    } else if target_secs == current_secs {
        0
    } else {
        86400 - (current_secs - target_secs)
    }
}

/// Seconds elapsed since `target` (forward direction, wrapping at midnight).
fn elapsed_since(current: NaiveTime, target: NaiveTime) -> u64 {
    let current_secs = time_to_secs(current);
    let target_secs = time_to_secs(target);

    if current_secs >= target_secs {
        current_secs - target_secs
    } else {
        86400 - (target_secs - current_secs)
    }
}

fn time_to_secs(t: NaiveTime) -> u64 {
    t.hour() as u64 * 3600 + t.minute() as u64 * 60 + t.second() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seconds_until() {
        let t1 = NaiveTime::from_hms_opt(11, 59, 50).unwrap();
        let t2 = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        assert_eq!(seconds_until_time(t1, t2), 10);
    }

    #[test]
    fn test_elapsed_since() {
        let current = NaiveTime::from_hms_opt(12, 0, 5).unwrap();
        let target = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        assert_eq!(elapsed_since(current, target), 5);
    }

    #[test]
    fn test_format_countdown() {
        let state = DisplayState::Countdown {
            name: "Team sync".into(),
            seconds_left: 67,
            highlight: false,
            sound: String::new(),
            sounds_to_play: vec![],
        };
        assert_eq!(state.menu_bar_text(), "((•)) Team sync in 1:07");
    }

    #[test]
    fn test_format_live() {
        let state = DisplayState::Live {
            name: "Team sync".into(),
            sound: String::new(),
        };
        assert_eq!(state.menu_bar_text(), "((•)) Team sync is live!");
    }

    #[test]
    fn test_format_short_countdown() {
        let state = DisplayState::Countdown {
            name: "Standup".into(),
            seconds_left: 7,
            highlight: true,
            sound: String::new(),
            sounds_to_play: vec![],
        };
        assert_eq!(state.menu_bar_text(), "((•)) Standup in 0:07");
    }

    #[test]
    fn test_format_time_until() {
        assert_eq!(format_time_until(0), "live!");
        assert_eq!(format_time_until(45), "0:45");
        assert_eq!(format_time_until(90), "1:30");
        assert_eq!(format_time_until(3599), "59:59");
        assert_eq!(format_time_until(3600), "1h 0m");
        assert_eq!(format_time_until(4500), "1h 15m");
        assert_eq!(format_time_until(86400), "1d 0h");
        assert_eq!(format_time_until(90000), "1d 1h");
    }
}
