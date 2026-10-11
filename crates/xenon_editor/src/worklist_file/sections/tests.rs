use super::*;
use crate::worklist_file::entries::parse;
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
fn append_preserves_crlf_in_the_middle_of_the_file() {
    let source = SECTIONED.replace('\n', "\r\n");
    let out = append(&source, &section(&source, "Now"), "New");
    assert!(out.contains("- [ ] A\r\n\r\n- [ ] New\r\n\r\n## Next"));
    assert_eq!(out.matches('\n').count(), out.matches("\r\n").count());
}

#[test]
fn append_still_refuses_an_unfinished_fence() {
    let source = "# W\n\n## Now\n\n```md\n";
    assert!(append_item(source, &Target::Top, &item("x"), false).is_err());
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
fn insert_section_into_an_empty_file_adds_the_title() {
    assert_eq!(
        insert_section("", None, "Now").unwrap().apply(""),
        "# Worklist\n\n## Now\n"
    );
}
