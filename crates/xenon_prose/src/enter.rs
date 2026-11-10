#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Enter {
    Continue(String),
    End,
}

/// What Enter does at the end of `line`, a list item's first line: continue
/// the list (next number, empty task box, same indent) or end it when the item
/// is empty. `None` when the line is not a list item.
pub fn list_enter(line: &str) -> Option<Enter> {
    let rest = line.trim_start();
    let indent = &line[..line.len() - rest.len()];
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    let (len, marker) = if digits == 0 {
        let c = rest.chars().next().filter(|c| "-*+".contains(*c))?;
        (1, c.to_string())
    } else if digits < 10 {
        let delim = rest[digits..]
            .chars()
            .next()
            .filter(|c| ".)".contains(*c))?;
        let n: u64 = rest[..digits].parse().ok()?;
        (digits + 1, format!("{}{delim}", n + 1))
    } else {
        return None;
    };
    let after = rest[len..].strip_prefix(' ')?;
    let task = ["[ ]", "[x]", "[X]"].iter().any(|b| {
        after
            .strip_prefix(b)
            .is_some_and(|r| r.is_empty() || r.starts_with(' '))
    });
    let body = if task { &after[3..] } else { after };
    Some(if body.trim().is_empty() {
        Enter::End
    } else {
        Enter::Continue(format!(
            "{indent}{marker} {}",
            if task { "[ ] " } else { "" }
        ))
    })
}
