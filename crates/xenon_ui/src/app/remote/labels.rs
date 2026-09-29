//! Device labels: sanitize what the phone sends; shorten for the Mac banner.

/// "iPhone · Home Screen" → "iPhone" for the terminal banner.
pub(crate) fn device_short_name(label: &str) -> &str {
    label.split(" · ").next().unwrap_or(label).trim()
}

pub(super) fn clean_label(label: &str) -> String {
    let label: String = label.chars().filter(|c| !c.is_control()).take(40).collect();
    let label = label.trim();
    if label.is_empty() {
        "Phone".to_string()
    } else {
        label.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_name_takes_the_device_part() {
        assert_eq!(device_short_name("iPhone · Home Screen"), "iPhone");
        assert_eq!(device_short_name("Pixel"), "Pixel");
    }
}
