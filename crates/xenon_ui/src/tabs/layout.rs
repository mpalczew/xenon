//! Which mixed-tab chips stay on the strip when the leaf is too wide.

pub(crate) const OVERFLOW_BTN: f32 = 44.;
const LABEL_MAX: f32 = 160.;
const CHIP_CHROME: f32 = 12. * 2. + 8. + 12. + 1.;
const CHAR_W: f32 = 7.2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackedTabs {
    pub visible: Vec<usize>,
    pub hidden: Vec<usize>,
}

pub(crate) fn chip_width(label: &str) -> f32 {
    CHIP_CHROME + (label.chars().count() as f32 * CHAR_W).min(LABEL_MAX)
}

/// Keep the active tab on the strip. Earlier tabs fill remaining room.
pub(crate) fn pack_tabs(widths: &[f32], active: usize, avail: f32) -> PackedTabs {
    if widths.is_empty() {
        return PackedTabs {
            visible: Vec::new(),
            hidden: Vec::new(),
        };
    }
    let total: f32 = widths.iter().sum();
    let room = if total > avail {
        (avail - OVERFLOW_BTN).max(0.)
    } else {
        avail
    };
    let active = active.min(widths.len() - 1);
    let mut visible = Vec::new();
    let mut hidden = Vec::new();
    let mut used = 0.;
    for (i, &w) in widths.iter().enumerate() {
        if i == active {
            while !visible.is_empty() && used + w > room {
                let drop = visible.pop().unwrap();
                used -= widths[drop];
                hidden.insert(0, drop);
            }
            visible.push(i);
            used += w;
        } else if used + w <= room {
            visible.push(i);
            used += w;
        } else {
            hidden.push(i);
        }
    }
    PackedTabs { visible, hidden }
}

#[cfg(test)]
mod tests {
    use super::{OVERFLOW_BTN, pack_tabs};

    #[test]
    fn all_visible_when_they_fit() {
        let packed = pack_tabs(&[80., 80., 80.], 0, 300.);
        assert!(packed.hidden.is_empty());
        assert_eq!(packed.visible, vec![0, 1, 2]);
    }

    #[test]
    fn hides_later_tabs() {
        let packed = pack_tabs(&[100., 100., 100., 100.], 0, 250.);
        assert_eq!(packed.visible, vec![0, 1]);
        assert_eq!(packed.hidden, vec![2, 3]);
    }

    #[test]
    fn keeps_active_on_strip() {
        let packed = pack_tabs(&[120., 120., 120., 120.], 3, 200.);
        assert!(packed.visible.contains(&3));
        assert!(!packed.hidden.contains(&3));
        let used: f32 = packed.visible.iter().map(|_| 120.).sum();
        assert!(used <= 200. - OVERFLOW_BTN + 0.01);
    }
}
