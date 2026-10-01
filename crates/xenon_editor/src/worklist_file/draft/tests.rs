use super::*;

fn item(title: &str, details: &str) -> WorkItem {
    WorkItem::new(title, details, true).unwrap()
}

fn land(draft: &mut ItemDraft, source: &str, item: &WorkItem) -> String {
    let change = draft.save(source, item).unwrap().unwrap();
    let updated = change.apply(source);
    draft.commit(&updated, &change);
    updated
}

#[test]
fn title_reads_the_saved_first_line() {
    let mut draft = ItemDraft::default();
    assert_eq!(draft.title(), None);
    land(&mut draft, "# Worklist\n", &item("Fix focus", "Right pane"));
    assert_eq!(draft.title(), Some("Fix focus"));
}

#[test]
fn first_save_appends_then_later_saves_rewrite_in_place() {
    let mut draft = ItemDraft::default();
    let source = "# Worklist\n\n- [ ] Old\n";
    let source = land(&mut draft, source, &item("F", ""));
    assert_eq!(source, "# Worklist\n\n- [ ] Old\n\n- [ ] F\n");
    let source = land(&mut draft, &source, &item("Fix", "why"));
    assert_eq!(source, "# Worklist\n\n- [ ] Old\n\n- [ ] Fix\n  - why\n");
    assert!(draft.save(&source, &item("Fix", "why")).unwrap().is_none());
}

#[test]
fn rewrite_keeps_checked_state_and_crlf() {
    let source = "- [x] First\r\n  - Detail\r\n\r\nNext note.\r\n";
    let range = entries(source)[0].range.clone();
    let mut draft = ItemDraft::existing(source, range, true);
    let updated = land(&mut draft, source, &item("Revised", "More"));
    assert_eq!(updated, "- [x] Revised\r\n  - More\r\n\r\nNext note.\r\n");
}

#[test]
fn follows_the_item_when_an_agent_edits_above_it() {
    let mut draft = ItemDraft::default();
    let source = "# Worklist\n";
    let source = land(&mut draft, source, &item("Mine", ""));
    let source = source.replace("# Worklist\n", "# Worklist\n\n- [ ] Agent\n");
    let updated = land(&mut draft, &source, &item("Mine!", ""));
    assert_eq!(updated, "# Worklist\n\n- [ ] Agent\n\n- [ ] Mine!\n");
}

#[test]
fn refuses_to_write_over_an_item_changed_elsewhere() {
    let mut draft = ItemDraft::default();
    let source = "# Worklist\n";
    let source = land(&mut draft, source, &item("Mine", ""));
    let source = source.replace("Mine", "Theirs");
    assert!(draft.save(&source, &item("Mine!", "")).is_err());
}

#[test]
fn removing_the_last_item_restores_the_source() {
    let mut draft = ItemDraft::default();
    let source = "# Worklist\n";
    let added = land(&mut draft, source, &item("Gone", ""));
    assert_eq!(draft.remove(&added).unwrap().unwrap().apply(&added), source);
}

#[test]
fn first_save_files_under_its_section_and_tracks_the_item() {
    let source = "# Worklist\n\n## Now\n\n- [ ] A\n\n## Next\n\n- [ ] B\n";
    let target = Target::Section {
        index: 0,
        title: "Now".into(),
    };
    let mut draft = {
        let mut draft = ItemDraft::default();
        draft.set_target(target);
        draft
    };
    let source = land(&mut draft, source, &item("F", ""));
    assert_eq!(
        source,
        "# Worklist\n\n## Now\n\n- [ ] A\n\n- [ ] F\n\n## Next\n\n- [ ] B\n"
    );
    let source = land(&mut draft, &source, &item("Fix", "why"));
    assert_eq!(
        source,
        "# Worklist\n\n## Now\n\n- [ ] A\n\n- [ ] Fix\n  - why\n\n## Next\n\n- [ ] B\n"
    );
    let removed = draft.remove(&source).unwrap().unwrap().apply(&source);
    assert_eq!(
        removed,
        "# Worklist\n\n## Now\n\n- [ ] A\n\n## Next\n\n- [ ] B\n"
    );
}
