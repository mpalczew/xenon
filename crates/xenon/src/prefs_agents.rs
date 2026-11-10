use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use xenon_settings::{skill_homes, skill_refresh_label, skill_subtitle};
use xenon_store::{SkillStatus, install_skill, remove_skill, skill_status};

use crate::prefs_pages::switch_row;

type Added = Rc<RefCell<Vec<gtk::Widget>>>;

pub(crate) fn agents(dialog: &crate::prefs::Shell) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .name("agents")
        .title("Agents")
        .icon_name("application-x-addon-symbolic")
        .build();
    let group = adw::PreferencesGroup::builder().title("Agents").build();
    page.add(&group);
    fill(&group, dialog, &Added::default());
    page
}

fn fill(group: &adw::PreferencesGroup, dialog: &crate::prefs::Shell, added: &Added) {
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
        button.connect_clicked(move |_| {
            let (group, dialog, added) = (group.clone(), dialog.clone(), added.clone());
            gtk::glib::idle_add_local_once(move || run(&group, &dialog, &added, true));
        });
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

fn run(group: &adw::PreferencesGroup, dialog: &crate::prefs::Shell, added: &Added, on: bool) {
    let message = match if on { install_skill() } else { remove_skill() } {
        Ok(()) if on => "Xenon skill installed".to_string(),
        Ok(()) => "Xenon skill removed".to_string(),
        Err(error) if on => format!("Couldn\u{2019}t install the skill: {error}"),
        Err(error) => format!("Couldn\u{2019}t remove the skill: {error}"),
    };
    dialog.add_toast(&message);
    fill(group, dialog, added);
}
