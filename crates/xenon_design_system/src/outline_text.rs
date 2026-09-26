//! Plain-text conversion for an outline. No view state.

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

pub fn details_from_points(points: &[OutlinePoint]) -> String {
    points
        .iter()
        .map(|point| format!("{}{}", "  ".repeat(point.depth as usize), point.text))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_details_stay_depth_zero() {
        let points = points_from_details("First point\nSecond point");
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].depth, 0);
        assert_eq!(points[1].text, "Second point");
        assert_eq!(details_from_points(&points), "First point\nSecond point");
    }

    #[test]
    fn indented_details_keep_depth() {
        let points = points_from_details("Parent\n  Child");
        assert_eq!(points[1].depth, 1);
        assert_eq!(points[1].text, "Child");
        assert_eq!(details_from_points(&points), "Parent\n  Child");
    }
}
