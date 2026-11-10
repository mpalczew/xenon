#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlinePoint {
    pub text: String,
    pub depth: u8,
}

pub fn points_from_details(details: &str) -> Vec<OutlinePoint> {
    details
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            let spaces = line
                .chars()
                .take_while(|character| *character == ' ')
                .count();
            let body = line[spaces..].trim();
            let text = body.strip_prefix("- ").unwrap_or(body).trim().to_owned();
            if text.is_empty() {
                return None;
            }
            Some(OutlinePoint {
                text,
                depth: u8::try_from(spaces / 2).unwrap_or(u8::MAX),
            })
        })
        .collect()
}
