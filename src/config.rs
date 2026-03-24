use chrono::{NaiveTime, Weekday};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_live_duration")]
    pub live_duration_secs: u64,

    #[serde(default)]
    pub events: Vec<EventConfig>,
}

fn default_live_duration() -> u64 {
    300 // 5 minutes
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventConfig {
    pub name: String,

    /// Time of day in "HH:MM" or "HH:MM:SS" format
    pub time: String,

    /// Days of the week: "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"
    /// If empty, runs every day.
    #[serde(default)]
    pub days: Vec<String>,

    /// Specific dates in "YYYY-MM-DD" format. If set, `days` is ignored.
    #[serde(default)]
    pub dates: Vec<String>,

    /// How many seconds before the event to start showing the countdown.
    #[serde(default = "default_countdown_start")]
    pub countdown_start_secs: u64,

    /// macOS system sound name for the "is live!" transition.
    /// Empty string or omitted = no sound.
    #[serde(default)]
    pub sound: String,

    /// Sounds to play during the countdown at specific seconds remaining.
    /// Each entry specifies a file path and the second at which to play.
    #[serde(default)]
    pub countdown_sounds: Vec<CountdownSound>,

    /// Whether to highlight (pill background) during countdown.
    #[serde(default = "default_true")]
    pub highlight: bool,

    /// Seconds before event at which to start highlighting.
    /// Defaults to 10 seconds.
    #[serde(default = "default_highlight_at")]
    pub highlight_at_secs: u64,
}

fn default_countdown_start() -> u64 {
    60
}

fn default_true() -> bool {
    true
}

fn default_highlight_at() -> u64 {
    10
}

/// A sound cue to play at a specific second during the countdown.
#[derive(Debug, Clone, Deserialize)]
pub struct CountdownSound {
    /// Path to the sound file (e.g. .aiff, .mp3, .wav, .m4a).
    /// Can also be a macOS system sound name (e.g. "Ping", "Glass").
    pub path: String,

    /// Seconds remaining at which to play this sound.
    /// E.g. `at_secs = 10` plays when the countdown shows 0:10.
    pub at_secs: u64,

    /// Volume from 0.0 to 1.0. Defaults to 1.0.
    #[serde(default = "default_volume")]
    pub volume: f32,
}

fn default_volume() -> f32 {
    1.0
}

impl EventConfig {
    pub fn parsed_time(&self) -> Option<NaiveTime> {
        NaiveTime::parse_from_str(&self.time, "%H:%M:%S")
            .or_else(|_| NaiveTime::parse_from_str(&self.time, "%H:%M"))
            .ok()
    }

    pub fn parsed_weekdays(&self) -> Vec<Weekday> {
        self.days
            .iter()
            .filter_map(|d| match d.to_lowercase().as_str() {
                "mon" | "monday" => Some(Weekday::Mon),
                "tue" | "tuesday" => Some(Weekday::Tue),
                "wed" | "wednesday" => Some(Weekday::Wed),
                "thu" | "thursday" => Some(Weekday::Thu),
                "fri" | "friday" => Some(Weekday::Fri),
                "sat" | "saturday" => Some(Weekday::Sat),
                "sun" | "sunday" => Some(Weekday::Sun),
                _ => None,
            })
            .collect()
    }

    pub fn parsed_dates(&self) -> Vec<chrono::NaiveDate> {
        self.dates
            .iter()
            .filter_map(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .collect()
    }
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => {
                    eprintln!("Loaded config from {}", path.display());
                    config
                }
                Err(e) => {
                    eprintln!("Failed to parse config {}: {e}", path.display());
                    Self::default_config()
                }
            },
            Err(_) => {
                eprintln!("No config at {}, creating example...", path.display());
                let config = Self::default_config();
                config.save_example(&path);
                config
            }
        }
    }

    fn default_config() -> Self {
        Config {
            live_duration_secs: 300,
            events: vec![EventConfig {
                name: "Team sync".into(),
                time: "12:00".into(),
                days: vec![
                    "Mon".into(),
                    "Tue".into(),
                    "Wed".into(),
                    "Thu".into(),
                    "Fri".into(),
                ],
                dates: vec![],
                countdown_start_secs: 60,
                sound: "Ping".into(),
                countdown_sounds: vec![],
                highlight: true,
                highlight_at_secs: 10,
            }],
        }
    }

    fn save_example(&self, path: &PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let example = r#"# Menubar Countdown configuration
# Events will show a countdown in the macOS menu bar.

# How long to show "is live!" after the event time (seconds)
live_duration_secs = 300

[[events]]
name = "Team sync"
time = "12:00"
days = ["Mon", "Tue", "Wed", "Thu", "Fri"]
# dates = ["2026-03-25", "2026-04-01"]  # Use specific dates instead of days
countdown_start_secs = 60   # Start showing countdown 60s before
sound = "Ping"              # System sound or file path for the "is live!" moment
highlight = true            # Show pill background
highlight_at_secs = 10      # Highlight in the last 10 seconds

# Sound cues during countdown. Each plays once when seconds_left == at_secs.
# "path" can be a macOS system sound name OR an absolute file path.
[[events.countdown_sounds]]
path = "Tink"
at_secs = 10

[[events.countdown_sounds]]
path = "Tink"
at_secs = 5

[[events.countdown_sounds]]
path = "Tink"
at_secs = 4

[[events.countdown_sounds]]
path = "Tink"
at_secs = 3

[[events.countdown_sounds]]
path = "Tink"
at_secs = 2

[[events.countdown_sounds]]
path = "Tink"
at_secs = 1

# You can also use file paths:
# [[events.countdown_sounds]]
# path = "/Users/piotr/Sounds/beep.aiff"
# at_secs = 30
# volume = 0.5

[[events]]
name = "Standup"
time = "09:30"
days = ["Mon", "Tue", "Wed", "Thu", "Fri"]
countdown_start_secs = 120
sound = "Glass"
highlight = true
highlight_at_secs = 15

[[events.countdown_sounds]]
path = "/Users/piotr/Sounds/countdown.mp3"
at_secs = 10
volume = 0.8
"#;
        match std::fs::write(path, example) {
            Ok(()) => eprintln!("Wrote example config to {}", path.display()),
            Err(e) => eprintln!("Could not write example config: {e}"),
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("menubar-countdown")
        .join("config.toml")
}
