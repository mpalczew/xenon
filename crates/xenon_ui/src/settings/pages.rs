//! Settings pages that are plain lists: Fonts, Editor, Terminal, Language Servers.

use std::rc::Rc;

use gpui::{App, Window};
use xenon_settings::TerminalAutoClose;

use super::row::{Control, Field, Group, Run, SettingRow};
use crate::dropdown::{DropdownId, SizeTarget};

const EXIT_MODES: [TerminalAutoClose; 5] = [
    TerminalAutoClose::Off,
    TerminalAutoClose::Immediate,
    TerminalAutoClose::After1s,
    TerminalAutoClose::After3s,
    TerminalAutoClose::After5s,
];

pub(super) fn fonts() -> Vec<Group> {
    let face = |id, label, family, size| {
        SettingRow::new(id, label, Control::Font { family, size }).keywords("font size family")
    };
    vec![Group::untitled(vec![
        face("font-ui", "Interface", DropdownId::Ui, SizeTarget::Ui)
            .detail("Sidebar, tabs, and menus"),
        face(
            "font-editor",
            "Editor",
            DropdownId::Editor,
            SizeTarget::Editor,
        )
        .detail("Monospaced faces only"),
        face(
            "font-terminal",
            "Terminal",
            DropdownId::Terminal,
            SizeTarget::Terminal,
        )
        .detail("Monospaced faces only"),
    ])]
}

pub(super) fn editor(cx: &App) -> Vec<Group> {
    vec![Group::untitled(vec![
        switch_row(
            "line-numbers",
            "Line numbers",
            xenon_settings::show_line_numbers(cx),
            xenon_settings::toggle_line_numbers,
        )
        .detail("Show row numbers in text editors")
        .keywords("gutter"),
        switch_row(
            "vim-mode",
            "Vim mode",
            xenon_settings::vim_mode(cx),
            xenon_settings::toggle_vim_mode,
        )
        .detail("Modal editing in the text editor")
        .keywords("modal keys"),
        switch_row(
            "wrap-prose",
            "Wrap prose",
            xenon_settings::wrap_prose(cx),
            xenon_settings::toggle_wrap_prose,
        )
        .detail("Soft wrap Markdown and plain text. The file is not rewritten.")
        .keywords("soft wrap lines"),
        switch_row(
            "wrap-code",
            "Wrap code",
            xenon_settings::wrap_code(cx),
            xenon_settings::toggle_wrap_code,
        )
        .detail("Soft wrap every other file")
        .keywords("soft wrap lines"),
    ])]
}

fn switch_row(id: &'static str, label: &'static str, on: bool, toggle: fn(&mut App)) -> SettingRow {
    let toggle: Run = Rc::new(move |_: &mut Window, cx: &mut App| {
        toggle(cx);
        xenon_settings::save(cx);
        xenon_terminal::refresh_windows(cx);
    });
    SettingRow::new(id, label, Control::Switch { on, toggle })
}

pub(super) fn terminal(cx: &App) -> Vec<Group> {
    let current = xenon_settings::terminal_auto_close(cx);
    let selected = EXIT_MODES.iter().position(|m| *m == current).unwrap_or(0);
    vec![Group::untitled(vec![
        SettingRow::new(
            "terminal-exit",
            "When the process exits",
            Control::Choice {
                options: vec![
                    "Keep open".into(),
                    "Close".into(),
                    "1s".into(),
                    "3s".into(),
                    "5s".into(),
                ],
                selected,
                pick: Rc::new(|index, _, cx| set_auto_close(EXIT_MODES[index], cx)),
            },
        )
        .detail(format!(
            "New terminal tabs: {}",
            current.label().to_lowercase()
        ))
        .keywords("close tab auto shell"),
    ])]
}

fn set_auto_close(mode: TerminalAutoClose, cx: &mut App) {
    let mut settings = xenon_settings::snapshot(cx);
    settings.terminal_auto_close = mode;
    xenon_settings::apply(&settings, cx);
    xenon_settings::save(cx);
}

pub(super) fn language_servers() -> Vec<Group> {
    let lsp = xenon_store::load_settings().unwrap_or_default().lsp;
    let enabled = lsp.enabled;
    vec![Group::untitled(vec![
        SettingRow::new(
            "lsp-enabled",
            "Language servers",
            Control::Switch {
                on: enabled,
                toggle: Rc::new(|_, cx| {
                    crate::app::settings_set_lsp(|lsp| lsp.enabled = !lsp.enabled, cx)
                }),
            },
        )
        .detail("Go to definition, highlights, and diagnostics for Rust and TypeScript")
        .keywords("lsp"),
        command_row(
            Field::RustServer,
            "Rust server",
            lsp.rust.command.as_deref(),
            "rust-analyzer",
        ),
        command_row(
            Field::TypeScriptServer,
            "TypeScript server",
            lsp.typescript.command.as_deref(),
            "typescript-language-server or vtsls",
        ),
    ])]
}

fn command_row(
    field: Field,
    label: &'static str,
    command: Option<&str>,
    automatic: &'static str,
) -> SettingRow {
    let value = command.unwrap_or_default().to_string();
    let shown = if value.is_empty() {
        format!("Automatic · {automatic}")
    } else {
        value.clone()
    };
    let id = match field {
        Field::RustServer => "lsp-rust",
        _ => "lsp-typescript",
    };
    SettingRow::new(
        id,
        label,
        Control::Field {
            field,
            value,
            shown: shown.into(),
        },
    )
    .detail("Empty finds it on PATH")
    .keywords("lsp command path")
}

/// Save an edited server command (empty = automatic) and restart servers.
pub(super) fn set_server_command(field: Field, value: String, cx: &mut App) {
    let command = Some(value.trim().to_string()).filter(|c| !c.is_empty());
    crate::app::settings_set_lsp(
        move |lsp| match field {
            Field::RustServer => lsp.rust.command = command,
            Field::TypeScriptServer => lsp.typescript.command = command,
            Field::RemoteHostname => {}
        },
        cx,
    );
}
