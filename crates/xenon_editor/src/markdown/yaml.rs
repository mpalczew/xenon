//! Simple YAML frontmatter key/value extraction (not a full YAML parser).

/// Parse simple `key: value` YAML (including indented multi-line values).
/// Returns `None` when the body is not a flat list of pairs.
pub(super) fn split_simple_yaml_pairs(body: &str) -> Option<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_val = String::new();

    for line in body.lines() {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            current_key.as_ref()?;
            let piece = line.trim();
            if piece.is_empty() {
                continue;
            }
            if !current_val.is_empty() {
                current_val.push(' ');
            }
            current_val.push_str(piece);
            continue;
        }

        if let Some(key) = current_key.take() {
            let value = current_val.trim().to_string();
            if value.is_empty() {
                return None;
            }
            pairs.push((key, value));
            current_val.clear();
        }

        let (key, rest) = line.split_once(':')?;
        let key = key.trim();
        if key.is_empty() || key.chars().any(char::is_whitespace) {
            return None;
        }
        current_key = Some(key.to_string());

        let rest = rest.trim();
        // Drop common block/fold markers; keep any remaining same-line value.
        let rest = rest
            .strip_prefix(">-")
            .or_else(|| rest.strip_prefix("|-"))
            .or_else(|| rest.strip_prefix('>'))
            .or_else(|| rest.strip_prefix('|'))
            .unwrap_or(rest)
            .trim();
        if !rest.is_empty() {
            current_val = rest.to_string();
        }
    }

    if let Some(key) = current_key {
        let value = current_val.trim().to_string();
        if value.is_empty() {
            return None;
        }
        pairs.push((key, value));
    }

    if pairs.is_empty() { None } else { Some(pairs) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_pairs_single_line() {
        let pairs = split_simple_yaml_pairs("name: brainstorming\ndescription: Open space.\n")
            .expect("pairs");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("name".into(), "brainstorming".into()));
        assert_eq!(pairs[1], ("description".into(), "Open space.".into()));
    }

    #[test]
    fn yaml_pairs_multiline_description() {
        let body =
            "name: brainstorming\ndescription:\n  Open the problem space before\n  narrowing it.\n";
        let pairs = split_simple_yaml_pairs(body).expect("pairs");
        assert_eq!(pairs[0].0, "name");
        assert_eq!(pairs[1].0, "description");
        assert!(pairs[1].1.contains("Open the problem space"));
        assert!(pairs[1].1.contains("narrowing it."));
    }

    #[test]
    fn yaml_pairs_folded_marker() {
        let body = "name: hygiene\ndescription: >-\n  Codebase hygiene scan.\n";
        let pairs = split_simple_yaml_pairs(body).expect("pairs");
        assert_eq!(pairs[0].1, "hygiene");
        assert_eq!(pairs[1].1, "Codebase hygiene scan.");
    }

    #[test]
    fn yaml_pairs_rejects_orphan_indent() {
        assert!(split_simple_yaml_pairs("  just indented\n").is_none());
    }

    #[test]
    fn yaml_pairs_rejects_empty_value() {
        assert!(split_simple_yaml_pairs("name:\n").is_none());
    }
}
