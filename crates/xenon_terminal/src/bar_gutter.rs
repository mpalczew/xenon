const BAR_GLYPHS: [char; 11] = ['▎', '▍', '▌', '▋', '▊', '▉', '█', '│', '┃', '║', '|'];

/// Strip a quote bar (whitespace, one bar glyph, optional space) that prefixes
/// every non-empty line. Skipped when any line also ends in a bar glyph (box,
/// Markdown table, cursor block), where other logic owns the layout.
pub(super) fn strip(lines: &mut [String]) -> bool {
    let mut bar = None;
    for line in lines.iter().filter(|l| !l.trim().is_empty()) {
        let first = line
            .trim_start()
            .chars()
            .next()
            .filter(|c| BAR_GLYPHS.contains(c));
        let Some(first) = first else { return false };
        if *bar.get_or_insert(first) != first
            || line.trim_start()[first.len_utf8()..].ends_with(BAR_GLYPHS)
        {
            return false;
        }
    }
    let Some(bar) = bar else { return false };
    for line in lines.iter_mut().filter(|l| !l.trim().is_empty()) {
        let rest = &line.trim_start()[bar.len_utf8()..];
        *line = rest.strip_prefix(' ').unwrap_or(rest).to_string();
    }
    true
}

/// Join consecutive text lines into paragraphs, except before list items.
pub(super) fn mark_hard_wraps(lines: &mut [(String, bool)]) {
    for i in 0..lines.len().saturating_sub(1) {
        let next = lines[i + 1].0.trim_start();
        lines[i].1 = !lines[i].0.is_empty() && !next.is_empty() && !is_list_item(next);
    }
}

fn is_list_item(line: &str) -> bool {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    let rest = &line[digits..];
    ["- ", "* ", "• ", "+ "].iter().any(|m| line.starts_with(m))
        || (digits > 0 && (rest.starts_with(". ") || rest.starts_with(") ")))
}

#[cfg(test)]
mod tests {
    use crate::clipboard::clean_agent_output;

    #[test]
    fn clean_agent_output_strips_quote_bar_and_unwraps() {
        let sample = "  ▎ Stop your WhatsApp/wacli sync and disable every routine that uses it. Then zip your memory folder (all memory/log files), all of /workspace\n  ▎ including wacli-store, and your Status and Queue board files. Save it to my Mac's Downloads as grok-rescue-2026-10-01.zip. Reply with the file name\n  ▎ and confirm wacli is stopped.";
        assert_eq!(
            clean_agent_output(sample),
            "Stop your WhatsApp/wacli sync and disable every routine that uses it. Then zip your memory folder (all memory/log files), all of /workspace including wacli-store, and your Status and Queue board files. Save it to my Mac's Downloads as grok-rescue-2026-10-01.zip. Reply with the file name and confirm wacli is stopped."
        );
    }

    #[test]
    fn clean_agent_output_quote_bar_keeps_paragraphs_and_lists() {
        assert_eq!(
            clean_agent_output(
                "  ▎ first para\n  ▎ continues\n  ▎\n  ▎ second para\n  ▎ - item a\n  ▎ - item b"
            ),
            "first para continues\n\nsecond para\n- item a\n- item b"
        );
    }

    #[test]
    fn clean_agent_output_leaves_markdown_table_and_mixed_bars() {
        let table = "| a | b |\n|---|---|\n| 1 | 2 |";
        assert_eq!(clean_agent_output(table), table);
        assert_eq!(clean_agent_output("| grep foo\nplain"), "| grep foo\nplain");
    }
}
