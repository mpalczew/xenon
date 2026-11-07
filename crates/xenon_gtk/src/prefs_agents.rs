//! Settings › Agents (the managed skill) and › Language Servers. The text and
//! the command rule come from `xenon_settings`.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use xenon_settings::{
    LspServer, set_lsp_command, skill_homes, skill_refresh_label, skill_subtitle,
};
use xenon_store::{SkillStatus, install_skill, remove_skill, skill_status};

use crate::prefs_pages::switch_row;

type Added = Rc<RefCell<Vec<gtk::Widget>>>;

pub(crate) fn agents(dialog: &adw::PreferencesDialog) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .name("agents")
        .title("Agents")
        .icon_name("application-x-addon-symbolic")
        .build();
    let group = adw::PreferencesGroup::new();
    page.add(&group);
    fill(&group, dialog, &Added::default());
    page
}

/// Rebuild the group's rows from the skill's current state.
fn fill(group: &adw::PreferencesGroup, dialog: &adw::PreferencesDialog, added: &Added) {
    for old in added.borrow_mut().drain(..) {
        group.remove(&old);
    }
    let status = skill_status();
    let mut rows: Vec<gtk::Widget> = Vec::new();
    let toggle = {
        let (group, dialog, added) = (group.clone(), dialog.clone(), added.clone());
        switch_row(
            "Agent skill",
            &skill_subtitle(&status),
            !matches!(status, SkillStatus::Missing),
            move |on| {
                let installed = !matches!(skill_status(), SkillStatus::Missing);
                if on != installed {
                    run(&group, &dialog, &added, on);
                }
            },
        )
    };
    rows.push(toggle.upcast());
    let homes = adw::ActionRow::builder()
        .title("Installed in")
        .subtitle(skill_homes(&status))
        .subtitle_lines(3)
        .build();
    homes.add_css_class("monospace");
    rows.push(homes.upcast());
    if let Some(label) = skill_refresh_label(&status) {
        let button = gtk::Button::with_label(label);
        button.add_css_class("suggested-action");
        button.set_valign(gtk::Align::Center);
        let (group, dialog, added) = (group.clone(), dialog.clone(), added.clone());
        button.connect_clicked(move |_| run(&group, &dialog, &added, true));
        let row = adw::ActionRow::builder()
            .title("Bundled skill")
            .subtitle("Replace your copy with the one shipped in this build")
            .build();
        row.add_suffix(&button);
        rows.push(row.upcast());
    }
    for row in &rows {
        group.add(row);
    }
    *added.borrow_mut() = rows;
}

fn run(group: &adw::PreferencesGroup, dialog: &adw::PreferencesDialog, added: &Added, on: bool) {
    let message = match if on { install_skill() } else { remove_skill() } {
        Ok(()) if on => "Xenon skill installed".to_string(),
        Ok(()) => "Xenon skill removed".to_string(),
        Err(error) => format!("Could not change the skill: {error}"),
    };
    dialog.add_toast(adw::Toast::new(&message));
    fill(group, dialog, added);
}

pub(crate) fn servers() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .name("servers")
        .title("Language Servers")
        .icon_name("system-run-symbolic")
        .build();
    let lsp = xenon_store::load_settings().unwrap_or_default().lsp;
    let group = adw::PreferencesGroup::builder()
        .description(
            "Leave a command empty to find it on PATH (rust-analyzer; typescript-language-server or vtsls). The GTK editor does not run language servers yet; the choices are saved.",
        )
        .build();
    group.add(&switch_row(
        "Language servers",
        "Go to definition, highlights, and diagnostics for Rust and TypeScript",
        lsp.enabled,
        |on| save(|l| l.enabled = on),
    ));
    for (title, server, command) in [
        ("Rust server", LspServer::Rust, &lsp.rust.command),
        (
            "TypeScript server",
            LspServer::TypeScript,
            &lsp.typescript.command,
        ),
    ] {
        let row = adw::EntryRow::builder()
            .title(title)
            .text(command.as_deref().unwrap_or_default())
            .show_apply_button(true)
            .build();
        row.connect_apply(move |row| {
            let text = row.text();
            save(|l| set_lsp_command(l, server, &text));
        });
        group.add(&row);
    }
    page.add(&group);
    page
}

fn save(change: impl FnOnce(&mut xenon_store::LspSettings)) {
    if let Err(error) = xenon_store::update_settings(|s| change(&mut s.lsp)) {
        eprintln!("[xg] save language server settings: {error}");
    }
}
