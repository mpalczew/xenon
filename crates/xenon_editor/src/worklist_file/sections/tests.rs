use super::*;
use crate::worklist_file::entries::{entries, parse};
use crate::worklist_file::{WorkItem, append_item};

const SECTIONED: &str =
    "# Worklist\n\nLoose.\n\n## Now\n\n- [ ] A\n\n## Next\n\n- [ ] B\n\n### Deeper\n\n- [x] C\n";

fn item(title: &str) -> WorkItem {
    WorkItem::new(title, "", true).unwrap()
}

fn titles(source: &str) -> Vec<String> {
    parse(source)
        .sections
        .into_iter()
        .map(|s| s.title)
        .collect()
}

#[test]
fn first_h1_is_the_title_and_later_h1_h2_are_sections() {
    assert_eq!(
        titles("# Worklist\n\n## Now\n\n# Later\n"),
        ["Now", "Later"]
    );
    assert!(titles("# Worklist\n\n- [ ] A\n").is_empty());
}

#[test]
fn deeper_headings_and_headings_in_lists_are_content() {
    assert_eq!(titles(SECTIONED), ["Now", "Next"]);
    assert!(titles("# W\n\n- item\n  # not a section\n\n> ## quoted\n").is_empty());
}

#[test]
fn entries_know_their_section() {
    let rows = entries(SECTIONED);
    let by: Vec<_> = rows.iter().map(|r| (r.title.as_str(), r.section)).collect();
    assert_eq!(
        by,
        [
            ("Loose.", None),
            ("A", Some(0)),
            ("B", Some(1)),
            ("C", Some(1))
        ]
    );
}

#[test]
fn a_file_without_sections_is_all_top() {
    let rows = entries("# Worklist\n\n- [ ] A\n- B\n");
    assert!(rows.iter().all(|r| r.section.is_none()));
}

#[test]
fn capture_targets_list_top_only_when_something_is_there() {
    let labels = |s: &str| {
        capture_targets(s)
            .iter()
            .map(|t| t.label().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(labels(SECTIONED), ["Top", "Now", "Next"]);
    assert_eq!(labels("# W\n\n## Now\n\n## Next\n"), ["Now", "Next"]);
    assert_eq!(labels("# W\n\n- [ ] A\n"), ["Top"]);
    assert_eq!(labels(""), ["Top"]);
}

fn section(source: &str, title: &str) -> Target {
    let sections = parse(source).sections;
    let index = sections.iter().position(|s| s.title == title).unwrap();
    Target::of(&sections, Some(index))
}

fn append(source: &str, target: &Target, title: &str) -> String {
    let insertion = append_block(
        source,
        target,
        &item(title).markdown(false, newline_of(source)),
    )
    .unwrap();
    let mut out = source.to_owned();
    out.replace_range(insertion.range, &insertion.text);
    out
}

#[test]
fn append_lands_at_the_end_of_a_middle_section() {
    let out = append(SECTIONED, &section(SECTIONED, "Now"), "New");
    assert_eq!(
        out,
        "# Worklist\n\nLoose.\n\n## Now\n\n- [ ] A\n\n- [ ] New\n\n## Next\n\n- [ ] B\n\n### Deeper\n\n- [x] C\n"
    );
}

#[test]
fn append_lands_at_the_end_of_the_last_section_and_the_top() {
    let last = append(SECTIONED, &section(SECTIONED, "Next"), "New");
    assert!(last.ends_with("- [x] C\n\n- [ ] New\n"));
    let top = append(SECTIONED, &Target::Top, "New");
    assert!(top.starts_with("# Worklist\n\nLoose.\n\n- [ ] New\n\n## Now\n"));
}

#[test]
fn append_into_an_empty_section_and_with_no_sections() {
    let source = "# Worklist\n\n## Now\n\n## Next\n\n- [ ] B\n";
    let out = append(source, &section(source, "Now"), "New");
    assert_eq!(
        out,
        "# Worklist\n\n## Now\n\n- [ ] New\n\n## Next\n\n- [ ] B\n"
    );
    assert_eq!(
        append("# Worklist\n\n- [ ] A\n", &Target::Top, "N"),
        "# Worklist\n\n- [ ] A\n\n- [ ] N\n"
    );
}

#[test]
fn append_before_a_heading_that_opens_the_file() {
    let source = "## Now\n\n- [ ] A\n";
    assert_eq!(
        append(source, &Target::Top, "N"),
        "- [ ] N\n\n## Now\n\n- [ ] A\n"
    );
}

#[test]
fn append_preserves_crlf_in_the_middle_of_the_file() {
    let source = SECTIONED.replace('\n', "\r\n");
    let out = append(&source, &section(&source, "Now"), "New");
    assert!(out.contains("- [ ] A\r\n\r\n- [ ] New\r\n\r\n## Next"));
    assert_eq!(out.matches('\n').count(), out.matches("\r\n").count());
}

#[test]
fn append_follows_a_renamed_or_renumbered_section() {
    let target = section(SECTIONED, "Next");
    let moved = SECTIONED.replace("## Now\n\n- [ ] A\n\n", "");
    let out = append(&moved, &target, "New");
    assert!(out.contains("- [ ] B"));
    assert!(append_block(&moved, &section(SECTIONED, "Now"), "x").is_err());
}

#[test]
fn append_still_refuses_an_unfinished_fence() {
    let source = "# W\n\n## Now\n\n```md\n";
    assert!(append_item(source, &Target::Top, &item("x"), false).is_err());
}

#[test]
fn rename_replaces_only_the_heading_text() {
    let edit = rename_section(SECTIONED, &section(SECTIONED, "Now"), "Today").unwrap();
    assert_eq!(
        edit.apply(SECTIONED),
        SECTIONED.replace("## Now", "## Today")
    );
    let crlf = "# W\r\n\r\n#  Old  \r\n\r\n- a\r\n";
    let edit = rename_section(crlf, &section(crlf, "Old"), "New").unwrap();
    assert_eq!(edit.apply(crlf), "# W\r\n\r\n#  New  \r\n\r\n- a\r\n");
    assert!(rename_section(SECTIONED, &section(SECTIONED, "Now"), "a\nb").is_err());
}

#[test]
fn only_empty_sections_delete() {
    let source = "# W\n\n## A\n\n- [ ] x\n\n## Empty\n\n## Last\n";
    let edit = delete_empty_section(source, &section(source, "Empty")).unwrap();
    assert_eq!(edit.apply(source), "# W\n\n## A\n\n- [ ] x\n\n## Last\n");
    let edit = delete_empty_section(source, &section(source, "Last")).unwrap();
    assert_eq!(edit.apply(source), "# W\n\n## A\n\n- [ ] x\n\n## Empty\n");
    assert!(delete_empty_section(source, &section(source, "A")).is_err());
}

#[test]
fn insert_section_after_a_section_at_top_and_at_the_end() {
    let source = "# W\n\n- [ ] top\n\n## A\n\n- [ ] x\n\n## B\n";
    let after_a = insert_section(source, Some(&section(source, "A")), "New").unwrap();
    assert_eq!(
        after_a.apply(source),
        "# W\n\n- [ ] top\n\n## A\n\n- [ ] x\n\n## New\n\n## B\n"
    );
    let after_top = insert_section(source, Some(&Target::Top), "New").unwrap();
    assert!(
        after_top
            .apply(source)
            .starts_with("# W\n\n- [ ] top\n\n## New\n\n## A\n")
    );
    let at_end = insert_section(source, None, "New").unwrap();
    assert!(at_end.apply(source).ends_with("## B\n\n## New\n"));
    let crlf = source.replace('\n', "\r\n");
    let edit = insert_section(&crlf, None, "New").unwrap().apply(&crlf);
    assert_eq!(edit.matches('\n').count(), edit.matches("\r\n").count());
}

#[test]
fn insert_section_into_an_empty_file_adds_the_title() {
    assert_eq!(
        insert_section("", None, "Now").unwrap().apply(""),
        "# Worklist\n\n## Now\n"
    );
}
