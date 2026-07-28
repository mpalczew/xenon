//! Content layout: binary pane tree of mixed tab stacks (terminals + editors).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::session::{OpenEditor, Point, TerminalState};

/// Max split depth root→leaf (at most 4 leaves on a path of length 3).
pub const MAX_NEST_DEPTH: u32 = 3;
/// Default first-child fraction when splitting.
pub const DEFAULT_SPLIT_RATIO: f32 = 0.5;
const RATIO_MIN: f32 = 0.15;
const RATIO_MAX: f32 = 0.85;

/// Stable id for a leaf pane within a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaneId(pub u64);

/// Stable id for a tab slot within a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TabId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitAxis {
    /// Side by side: first = left, second = right.
    Horizontal,
    /// Stacked: first = top, second = bottom.
    Vertical,
}

/// Edge of a leaf used for tab drop → split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl DropEdge {
    pub fn axis(self) -> SplitAxis {
        match self {
            Self::Left | Self::Right => SplitAxis::Horizontal,
            Self::Top | Self::Bottom => SplitAxis::Vertical,
        }
    }

    /// True when the dropped tab should land in the *first* child of the split.
    pub fn tab_in_first(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabState {
    Terminal {
        id: TabId,
        #[serde(default = "default_cwd")]
        cwd: PathBuf,
    },
    Editor {
        id: TabId,
        path: PathBuf,
        #[serde(default)]
        cursor: Point,
        #[serde(default)]
        scroll_top: u32,
    },
}

fn default_cwd() -> PathBuf {
    PathBuf::from(".")
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeafPane {
    pub id: PaneId,
    pub tabs: Vec<TabState>,
    #[serde(default)]
    pub active: usize,
}

impl LeafPane {
    pub(crate) fn new(id: PaneId, tabs: Vec<TabState>, active: usize) -> Self {
        let active = if tabs.is_empty() {
            0
        } else {
            active.min(tabs.len() - 1)
        };
        Self { id, tabs, active }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaneNode {
    Leaf(LeafPane),
    Split {
        axis: SplitAxis,
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

impl PaneNode {
    pub fn leaf_ids(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        self.collect_leaf_ids(&mut out);
        out
    }

    fn collect_leaf_ids(&self, out: &mut Vec<PaneId>) {
        match self {
            Self::Leaf(leaf) => out.push(leaf.id),
            Self::Split { first, second, .. } => {
                first.collect_leaf_ids(out);
                second.collect_leaf_ids(out);
            }
        }
    }

    fn clamp_ratios(&mut self) {
        match self {
            Self::Leaf(_) => {}
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => {
                *ratio = clamp_ratio(*ratio);
                first.clamp_ratios();
                second.clamp_ratios();
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ContentLayout {
    #[serde(default)]
    pub root: Option<PaneNode>,
    #[serde(default)]
    pub focused: Option<PaneId>,
}

impl ContentLayout {
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn leaf_ids(&self) -> Vec<PaneId> {
        self.root
            .as_ref()
            .map(PaneNode::leaf_ids)
            .unwrap_or_default()
    }

    /// Repair focused pointer and clamp ratios after load.
    pub fn normalize(&mut self) {
        if let Some(root) = &mut self.root {
            root.clamp_ratios();
            let leaves = root.leaf_ids();
            if leaves.is_empty() {
                self.root = None;
                self.focused = None;
                return;
            }
            if self.focused.is_none_or(|f| !leaves.contains(&f)) {
                self.focused = Some(leaves[0]);
            }
        } else {
            self.focused = None;
        }
    }
}

fn clamp_ratio(ratio: f32) -> f32 {
    if !ratio.is_finite() {
        return DEFAULT_SPLIT_RATIO;
    }
    ratio.clamp(RATIO_MIN, RATIO_MAX)
}

/// Build content layout from the pre-pane-tree session fields.
/// Inputs from the pre-pane-tree session JSON shape.
pub struct LegacySession {
    pub terminal_visible: bool,
    pub editor_visible: bool,
    pub terminal_width: f32,
    pub editors: Vec<OpenEditor>,
    pub active_editor: Option<usize>,
    pub terminal: TerminalState,
}

pub fn migrate_from_legacy(legacy: LegacySession) -> ContentLayout {
    let LegacySession {
        terminal_visible,
        editor_visible,
        terminal_width,
        editors,
        active_editor,
        terminal,
    } = legacy;
    let mut next_pane = 1u64;
    let mut next_tab = 1u64;
    let mut alloc_pane = || {
        let id = PaneId(next_pane);
        next_pane += 1;
        id
    };
    let mut alloc_tab = || {
        let id = TabId(next_tab);
        next_tab += 1;
        id
    };

    let make_term = |alloc_tab: &mut dyn FnMut() -> TabId| TabState::Terminal {
        id: alloc_tab(),
        cwd: terminal.cwd.clone(),
    };
    let make_editors = |alloc_tab: &mut dyn FnMut() -> TabId| -> Vec<TabState> {
        editors
            .iter()
            .map(|e| TabState::Editor {
                id: alloc_tab(),
                path: e.path.clone(),
                cursor: e.cursor,
                scroll_top: e.scroll_top,
            })
            .collect()
    };

    let has_editors = !editors.is_empty();
    let mut content = if terminal_visible && editor_visible && has_editors {
        let term_id = alloc_pane();
        let ed_id = alloc_pane();
        let ratio = clamp_ratio(terminal_width / (terminal_width + 400.0).max(1.0));
        let active = active_editor.unwrap_or(0);
        ContentLayout {
            root: Some(PaneNode::Split {
                axis: SplitAxis::Horizontal,
                ratio,
                first: Box::new(PaneNode::Leaf(LeafPane::new(
                    term_id,
                    vec![make_term(&mut alloc_tab)],
                    0,
                ))),
                second: Box::new(PaneNode::Leaf(LeafPane::new(
                    ed_id,
                    make_editors(&mut alloc_tab),
                    active,
                ))),
            }),
            focused: Some(term_id),
        }
    } else if has_editors && (editor_visible || !terminal_visible) {
        let ed_id = alloc_pane();
        let active = active_editor.unwrap_or(0);
        ContentLayout {
            root: Some(PaneNode::Leaf(LeafPane::new(
                ed_id,
                make_editors(&mut alloc_tab),
                active,
            ))),
            focused: Some(ed_id),
        }
    } else if terminal_visible || !has_editors {
        let term_id = alloc_pane();
        ContentLayout {
            root: Some(PaneNode::Leaf(LeafPane::new(
                term_id,
                vec![make_term(&mut alloc_tab)],
                0,
            ))),
            focused: Some(term_id),
        }
    } else {
        ContentLayout::default()
    };
    content.normalize();
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_both_visible() {
        let c = migrate_from_legacy(LegacySession {
            terminal_visible: true,
            editor_visible: true,
            terminal_width: 520.0,
            editors: vec![OpenEditor {
                path: PathBuf::from("x.rs"),
                cursor: Point::default(),
                scroll_top: 0,
            }],
            active_editor: Some(0),
            terminal: TerminalState::default(),
        });
        assert_eq!(c.leaf_ids().len(), 2);
    }

    #[test]
    fn content_round_trip_json() {
        let c = migrate_from_legacy(LegacySession {
            terminal_visible: true,
            editor_visible: true,
            terminal_width: 520.0,
            editors: vec![OpenEditor {
                path: PathBuf::from("a.rs"),
                cursor: Point::default(),
                scroll_top: 0,
            }],
            active_editor: Some(0),
            terminal: TerminalState::default(),
        });
        let json = serde_json::to_string(&c).unwrap();
        let parsed: ContentLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.leaf_ids().len(), 2);
    }

    #[test]
    fn normalize_repairs_focus_and_ratio() {
        let mut content = migrate_from_legacy(LegacySession {
            terminal_visible: true,
            editor_visible: true,
            terminal_width: f32::NAN,
            editors: vec![OpenEditor {
                path: PathBuf::from("a.rs"),
                cursor: Point::default(),
                scroll_top: 0,
            }],
            active_editor: Some(0),
            terminal: TerminalState::default(),
        });
        content.focused = Some(PaneId(999));
        content.normalize();
        assert!(content.leaf_ids().contains(&content.focused.unwrap()));
    }
}
