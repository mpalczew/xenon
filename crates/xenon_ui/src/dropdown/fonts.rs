//! Cached font family lists for Settings dropdowns.

use std::sync::Mutex;

use gpui::{App, SharedString};
use theme::FontFamilyCache;

use super::DropdownId;

fn is_mono_family_dropdown(id: DropdownId) -> bool {
    matches!(id, DropdownId::EditorFamily | DropdownId::TerminalFamily)
}

fn is_ui_family_dropdown(id: DropdownId) -> bool {
    matches!(id, DropdownId::UiFamily)
}

pub(super) fn family_option_label(id: DropdownId, name: &str) -> String {
    if is_mono_family_dropdown(id) {
        mono_family_label(name)
    } else if is_ui_family_dropdown(id) {
        xenon_settings::display_ui_family(name).to_string()
    } else {
        name.to_string()
    }
}

fn mono_cache() -> &'static Mutex<Option<Vec<SharedString>>> {
    static CACHE: Mutex<Option<Vec<SharedString>>> = Mutex::new(None);
    &CACHE
}

/// Known monospace faces to show before / without a full system scan.
/// Names must be real Core Text families (not marketing labels like "SF Mono").
fn seed_mono_families() -> Vec<SharedString> {
    [
        "Menlo",
        "Monaco",
        "Courier New",
        "Courier",
        "JetBrains Mono",
        "Fira Code",
        "Source Code Pro",
        "Cascadia Code",
        "IBM Plex Mono",
        "Inconsolata",
        "Hack",
    ]
    .into_iter()
    .map(SharedString::from)
    .collect()
}

/// Monospace families for Settings. Never runs a full system font scan on the
/// call path: returns the warm cache, else a small seed list. Use
/// [`warm_mono_font_families`] from a deferred task to fill the real cache.
pub(crate) fn mono_font_families(_cx: &App) -> Vec<SharedString> {
    let guard = mono_cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(cached) = guard.as_ref() {
        return cached.clone();
    }
    seed_mono_families()
}

/// Probe installed monospaced families and fill the cache. Safe to call from a
/// deferred task after Settings has opened (not from key handlers or paint).
pub(crate) fn warm_mono_font_families(cx: &App) {
    {
        let guard = mono_cache().lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_some() {
            return;
        }
    }
    let list = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        FontFamilyCache::global(cx)
            .list_font_families(cx)
            .into_iter()
            .filter(|name| xenon_settings::is_monospace_family(name.as_ref(), cx))
            .collect::<Vec<SharedString>>()
    }))
    .unwrap_or_else(|_| seed_mono_families());
    let mut guard = mono_cache().lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(if list.is_empty() {
            seed_mono_families()
        } else {
            list
        });
    }
}

fn ui_cache() -> &'static Mutex<Option<Vec<SharedString>>> {
    static CACHE: Mutex<Option<Vec<SharedString>>> = Mutex::new(None);
    &CACHE
}

fn seed_ui_families() -> Vec<SharedString> {
    [
        ".SystemUIFont",
        "Helvetica Neue",
        "Helvetica",
        "Arial",
        "Avenir Next",
        "Gill Sans",
        "Optima",
        "Palatino",
        "Times New Roman",
        "Georgia",
    ]
    .into_iter()
    .map(SharedString::from)
    .collect()
}

/// UI (proportional-friendly) families for Settings. Cache-first like mono.
pub(crate) fn ui_font_families(_cx: &App) -> Vec<SharedString> {
    let guard = ui_cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(cached) = guard.as_ref() {
        return cached.clone();
    }
    seed_ui_families()
}

/// Probe installed families for the UI font picker (deferred, not on paint).
pub(crate) fn warm_ui_font_families(cx: &App) {
    {
        let guard = ui_cache().lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_some() {
            return;
        }
    }
    let list = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        FontFamilyCache::global(cx)
            .list_font_families(cx)
            .into_iter()
            .collect::<Vec<SharedString>>()
    }))
    .unwrap_or_else(|_| seed_ui_families());
    let mut guard = ui_cache().lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(if list.is_empty() {
            seed_ui_families()
        } else {
            // Ensure system UI is always first / present.
            let mut out = list;
            let sys = SharedString::from(".SystemUIFont");
            if !out.iter().any(|n| n.as_ref() == sys.as_ref()) {
                out.insert(0, sys);
            }
            out
        });
    }
}

/// Friendly label for a stored mono family name (dropdown rows / trigger).
pub(crate) fn mono_family_label(family: &str) -> String {
    xenon_settings::display_mono_family(family).to_string()
}

pub(crate) fn format_size(size: f32) -> String {
    if (size - size.round()).abs() < 0.01 {
        format!("{}", size.round() as i32)
    } else {
        format!("{size:.1}")
    }
}
