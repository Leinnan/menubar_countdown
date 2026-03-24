#![cfg(target_os = "macos")]

mod config;
mod countdown;

use crate::config::{Config, CountdownSound};
use crate::countdown::{evaluate, format_time_until, seconds_until_next_occurrence, DisplayState};

use std::path::Path;
use std::time::{Duration, Instant};

use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

use tao::event_loop::{ControlFlow, EventLoopBuilder};

// For highlight: NSStatusBar / NSStatusItem / NSStatusBarButton / CALayer
use objc2::rc::Retained;
use objc2::{msg_send, AnyThread};
use objc2_app_kit::{NSSound, NSStatusBar, NSStatusItem};
use objc2_core_graphics::CGColor;
use objc2_foundation::{MainThreadMarker, NSString};
use objc2_quartz_core::CATransaction;

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct AppState {
    config: Config,
    tray: TrayIcon,
    /// Retained handle to the underlying NSStatusItem so we can call
    /// `button().highlight()` for the "pill" highlight effect.
    /// Obtained via the private `_statusItems` selector immediately after
    /// building the TrayIcon; `None` if that selector is unavailable.
    ns_status_item: Option<Retained<NSStatusItem>>,
    quit_id: MenuId,
    reload_id: MenuId,
    open_config_id: MenuId,
    /// One `MenuItem` per configured event, in the same order as `config.events`.
    /// Updated each tick to show time remaining.
    event_items: Vec<MenuItem>,
    was_live: bool,
    sound_played: bool,
    /// Toggles each second while highlight is active to create the blink effect.
    blink_on: bool,
}

// ---------------------------------------------------------------------------
// Menu construction
// ---------------------------------------------------------------------------

fn build_menu(config: &Config) -> (Menu, Vec<MenuItem>, MenuId, MenuId, MenuId) {
    let menu = Menu::new();

    // Header (disabled)
    menu.append(&MenuItem::new("Configured Events", false, None))
        .unwrap();

    let mut event_items = Vec::with_capacity(config.events.len());
    for event in &config.events {
        let days_str = if !event.dates.is_empty() {
            format!("dates: {}", event.dates.join(", "))
        } else if !event.days.is_empty() {
            event.days.join(", ")
        } else {
            "every day".into()
        };
        // Time-until placeholder filled in on first tick
        let title = format!("  {} — {} ({})", event.name, event.time, days_str,);
        let item = MenuItem::new(&title, false, None);
        menu.append(&item).unwrap();
        event_items.push(item);
    }

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // Reload Config ⌘R
    let reload_item = MenuItem::new("Reload Config", true, None);
    let reload_id = reload_item.id().clone();
    menu.append(&reload_item).unwrap();

    // Open Config
    let open_config_item = MenuItem::new("Open Config", true, None);
    let open_config_id = open_config_item.id().clone();
    menu.append(&open_config_item).unwrap();

    // Config path (disabled)
    let config_path_str = format!("Config: {}", config::config_path().display());
    menu.append(&MenuItem::new(&config_path_str, false, None))
        .unwrap();

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // Quit ⌘Q
    let quit_item = MenuItem::new("Quit", true, None);
    let quit_id = quit_item.id().clone();
    menu.append(&quit_item).unwrap();

    (menu, event_items, reload_id, open_config_id, quit_id)
}

// ---------------------------------------------------------------------------
// Highlight helpers
// ---------------------------------------------------------------------------

/// Apply (or remove) a vivid red pill highlight on the NSStatusBarButton.
///
/// When `active` is true:
///   - Enables `wantsLayer` on the button's NSView
///   - Sets the CALayer background to a red/orange color with rounded corners
///     (matching the reference screenshot)
///
/// When `active` is false:
///   - Clears the background color back to transparent
///   - Ensures the native `highlight(false)` is called so macOS restores
///     its own state
///
/// All Objective-C calls are wrapped in `unsafe` because we are calling raw
/// AppKit/QuartzCore selectors. We are on the main thread, and the button
/// object is known to be valid (caller checks `ns_status_item.is_some()`).
unsafe fn apply_highlight(button: &objc2_app_kit::NSStatusBarButton, active: bool) {
    // NSStatusBarButton inherits from NSView via NSButton → NSControl → NSView,
    // so all NSView methods (setWantsLayer, layer) are available via Deref.

    // Suppress macOS's own blue-pill highlight so it doesn't conflict.
    button.highlight(false);

    if active {
        // Make the view layer-backed so we can paint a custom background.
        button.setWantsLayer(true);

        // layer() is guaranteed non-nil after setWantsLayer(true).
        let layer = match button.layer() {
            Some(l) => l,
            None => return,
        };

        // Disable implicit CALayer animations so the blink is instant, not a fade.
        CATransaction::begin();
        CATransaction::setDisableActions(true);

        // Build a CGColor directly in the sRGB color space.
        // Red/orange matching the reference screenshot (R:1.0 G:0.27 B:0.18 A:1.0).
        let cg_color = CGColor::new_srgb(1.0, 0.27, 0.18, 1.0);
        layer.setBackgroundColor(Some(&cg_color));

        // Rounded corners for the pill shape (~5 pt radius matches the menu bar height).
        layer.setCornerRadius(5.0);

        CATransaction::commit();
    } else {
        // Remove the background and tear down the backing layer.
        if let Some(layer) = button.layer() {
            CATransaction::begin();
            CATransaction::setDisableActions(true);
            layer.setBackgroundColor(None);
            CATransaction::commit();
        }
        button.setWantsLayer(false);
    }
}

// ---------------------------------------------------------------------------
// Tick logic
// ---------------------------------------------------------------------------

fn on_tick(state: &mut AppState, mtm: MainThreadMarker) {
    let display = evaluate(&state.config);

    // Update menu bar title
    let text = display.menu_bar_text();
    if display.is_idle() {
        state.tray.set_title(Some("((•))"));
    } else {
        state.tray.set_title(Some(&text));
    }

    // Highlight effect via raw NSStatusItem button (best-effort).
    // ns_status_item is None when the private _statusItems selector is
    // unavailable; in that case we silently skip the highlight.
    if display.should_highlight() {
        // Toggle blink state each second while highlight is active.
        state.blink_on = !state.blink_on;
    } else {
        // Reset blink state when not highlighting so the first blink is always "on".
        state.blink_on = false;
    }

    if let Some(item) = &state.ns_status_item {
        if let Some(button) = item.button(mtm) {
            // Apply blinking red pill: show red on blink_on, clear on blink_off.
            unsafe { apply_highlight(&button, state.blink_on) };
        }
    }

    // Update each configured-event menu item with time remaining.
    for (event, menu_item) in state.config.events.iter().zip(state.event_items.iter()) {
        let days_str = if !event.dates.is_empty() {
            format!("dates: {}", event.dates.join(", "))
        } else if !event.days.is_empty() {
            event.days.join(", ")
        } else {
            "every day".into()
        };
        let time_str = match seconds_until_next_occurrence(event, &state.config) {
            Some(secs) => format_time_until(secs),
            None => "—".into(),
        };
        let title = format!(
            "  {} — {} ({}) · {}",
            event.name, event.time, days_str, time_str,
        );
        menu_item.set_text(&title);
    }

    // Sound dispatch
    match &display {
        DisplayState::Live { sound, .. } => {
            if !state.sound_played && !state.was_live {
                play_system_sound(sound);
                state.sound_played = true;
            }
            state.was_live = true;
        }
        DisplayState::Countdown { sounds_to_play, .. } => {
            for cue in sounds_to_play {
                play_countdown_sound(cue);
            }
            state.was_live = false;
            state.sound_played = false;
        }
        DisplayState::Idle => {
            state.was_live = false;
            state.sound_played = false;
        }
    }
}

// ---------------------------------------------------------------------------
// Sound playback (unchanged from original — still uses NSSound directly)
// ---------------------------------------------------------------------------

/// Play a system sound by name (e.g. "Ping", "Glass").
fn play_system_sound(name: &str) {
    if name.is_empty() {
        return;
    }
    if name.contains('/') {
        play_sound_file(name, 1.0);
        return;
    }
    let ns_name = NSString::from_str(name);
    if let Some(sound) = NSSound::soundNamed(&ns_name) {
        sound.play();
    } else {
        eprintln!("System sound not found: {name}");
    }
}

/// Play a countdown sound cue — either a system sound name or a file path.
fn play_countdown_sound(cue: &CountdownSound) {
    let path = &cue.path;
    if path.is_empty() {
        return;
    }
    if Path::new(path).is_absolute() || path.contains('/') {
        play_sound_file(path, cue.volume);
    } else {
        unsafe {
            use objc2::msg_send;
            let ns_name = NSString::from_str(path);
            if let Some(sound) = NSSound::soundNamed(&ns_name) {
                let _: () = msg_send![&sound, setVolume: cue.volume];
                sound.play();
            } else {
                eprintln!("System sound not found: {path}");
            }
        }
    }
}

/// Play a sound from a file path with the given volume.
fn play_sound_file(path: &str, volume: f32) {
    unsafe {
        use objc2::msg_send;
        let ns_path = NSString::from_str(path);
        let sound: Option<Retained<NSSound>> =
            msg_send![NSSound::alloc(), initWithContentsOfFile: &*ns_path, byReference: true];
        match sound {
            Some(s) => {
                let _: () = msg_send![&s, setVolume: volume];
                s.play();
            }
            None => {
                eprintln!("Could not load sound file: {path}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let mtm = MainThreadMarker::new().expect("must run on the main thread");

    let config = Config::load();
    eprintln!(
        "menubar-countdown: {} event(s) configured",
        config.events.len()
    );

    let (menu, event_items, reload_id, open_config_id, quit_id) = build_menu(&config);

    let tray = TrayIconBuilder::new()
        .with_title("((•))")
        .with_menu(Box::new(menu))
        .build()
        .expect("failed to create tray icon");

    // Grab the NSStatusItem that tray_icon just added so we can call
    // button().highlight() for the "pill" highlight effect.
    //
    // tray_icon doesn't expose the underlying NSStatusItem through its public
    // API. We retrieve it via the private -[NSStatusBar _statusItems] selector
    // (an NSArray) and take the last object, which is the one we just created.
    //
    // SAFETY: We are on the main thread. The TrayIcon was constructed above
    // and no other code runs between build() and this call, so `lastObject`
    // is guaranteed to be our item. We retain it to keep it alive alongside
    // AppState.
    // Retrieve the NSStatusItem tray_icon just created so we can call
    // button().highlight() for the "pill" highlight effect.
    //
    // NSStatusBar keeps items in a private NSConcretePointerArray accessed via
    // the private `_statusItems` selector. That array uses `pointerAtIndex:`
    // (not `lastObject`) with index = count - 1.
    //
    // SAFETY: main thread, called synchronously after TrayIconBuilder::build().
    let ns_status_item: Option<Retained<NSStatusItem>> = unsafe {
        let bar = NSStatusBar::systemStatusBar();
        let items: *mut objc2::runtime::AnyObject = msg_send![&*bar, _statusItems];
        if items.is_null() {
            eprintln!("warning: _statusItems unavailable; highlight disabled");
            None
        } else {
            let count: usize = msg_send![items, count];
            if count == 0 {
                eprintln!("warning: _statusItems is empty; highlight disabled");
                None
            } else {
                let last: *mut std::ffi::c_void = msg_send![items, pointerAtIndex: count - 1];
                let last = last as *mut NSStatusItem;
                if last.is_null() {
                    eprintln!("warning: _statusItems last pointer is null; highlight disabled");
                    None
                } else {
                    Retained::retain(last)
                }
            }
        }
    };

    let mut state = AppState {
        config,
        tray,
        ns_status_item,
        quit_id,
        reload_id,
        open_config_id,
        event_items,
        was_live: false,
        sound_played: false,
        blink_on: false,
    };

    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<()>::new().build();
    let tick_interval = Duration::from_secs(1);

    // Fire an immediate first tick before entering the loop
    on_tick(&mut state, mtm);
    let mut next_tick = Instant::now() + tick_interval;
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
        // Prevent the app from stealing focus from the currently active window
        // when launched. Without this, macOS briefly activates the process as a
        // regular app (flashing a window) before the Accessory policy takes hold.
        event_loop.set_activate_ignoring_other_apps(false);
    }
    event_loop.run(move |_event, _target, control_flow| {
        // Poll menu events
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == state.quit_id {
                *control_flow = ControlFlow::Exit;
                return;
            }
            if event.id == state.reload_id {
                state.config = Config::load();
                eprintln!("Config reloaded.");
            }
            if event.id == state.open_config_id {
                let path = config::config_path();
                if let Err(e) = open::that(&path) {
                    eprintln!("Failed to open config file {}: {e}", path.display());
                }
            }
        }

        let now = Instant::now();
        if now >= next_tick {
            on_tick(&mut state, mtm);
            next_tick = now + tick_interval;
        }

        *control_flow = ControlFlow::WaitUntil(next_tick);
    });
}
