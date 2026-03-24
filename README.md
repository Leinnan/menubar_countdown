# menubar-countdown

A macOS menu bar countdown timer for scheduled events, written in Rust.

Shows a countdown like `((•)) Team sync in 0:42` in your menu bar, transitioning to `((•)) Team sync is live!` when the time arrives — with timed sound cues and highlight.

## Build & Run

```bash
cargo build --release
./target/release/menubar-countdown
```

On first run, an example config is created at `~/.config/menubar-countdown/config.toml`.

## Configuration

```toml
live_duration_secs = 300

[[events]]
name = "Team sync"
time = "12:00"
days = ["Mon", "Tue", "Wed", "Thu", "Fri"]
countdown_start_secs = 60
sound = "Ping"             # "is live!" sound (system name or file path)
highlight = true
highlight_at_secs = 10

# Sound cues during countdown — plays at the specified second
[[events.countdown_sounds]]
path = "Tink"              # system sound name
at_secs = 10

[[events.countdown_sounds]]
path = "/Users/you/Sounds/beep.aiff"   # or a file path
at_secs = 5
volume = 0.8               # 0.0–1.0, defaults to 1.0

[[events.countdown_sounds]]
path = "Tink"
at_secs = 3

[[events.countdown_sounds]]
path = "Tink"
at_secs = 2

[[events.countdown_sounds]]
path = "Tink"
at_secs = 1
```

### Sound cue options

Each `[[events.countdown_sounds]]` entry has:

| Field | Required | Description |
|-------|----------|-------------|
| `path` | yes | macOS system sound name (`Tink`, `Ping`, `Glass`, `Pop`, etc.) or absolute path to a sound file (`.aiff`, `.mp3`, `.wav`, `.m4a`) |
| `at_secs` | yes | Seconds remaining when the sound plays (e.g. `10` = plays at `0:10`) |
| `volume` | no | Volume from `0.0` to `1.0` (default `1.0`) |

The `sound` field on the event itself plays once at the `0:00` → "is live!" transition.

### Available system sounds

Basso, Blow, Bottle, Frog, Funk, Glass, Hero, Morse, Ping, Pop, Purr, Sosumi, Submarine, Tink

### Menu bar states

| State | Display |
|-------|---------|
| Idle | `((•))` |
| Countdown | `((•)) Team sync in 1:07` |
| Highlight zone | same, with pill highlight |
| Live | `((•)) Team sync is live!` |

Click the icon to see events, reload config (⌘R), or quit (⌘Q).

## Launch at login

```bash
cat > ~/Library/LaunchAgents/com.menubar-countdown.plist << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.menubar-countdown</string>
    <key>ProgramArguments</key><array><string>/usr/local/bin/menubar-countdown</string></array>
    <key>RunAtLoad</key><true/>
</dict>
</plist>
PLIST

launchctl load ~/Library/LaunchAgents/com.menubar-countdown.plist
```

## Architecture

- No async runtime — `NSTimer` on the AppKit run loop
- Direct `objc2` 0.6 bindings (`define_class!`) — no heavy frameworks
- Thread-local state, no locks
- Config hot-reload via menu (⌘R)
