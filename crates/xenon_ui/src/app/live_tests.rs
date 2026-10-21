use super::*;

fn leaf(id: u64) -> LiveNode {
    LiveNode::Leaf(LiveLeaf {
        id: PaneId(id),
        tabs: Vec::new(),
        active: 0,
        parked: false,
    })
}

fn row(ids: &[u64]) -> LiveNode {
    let n = ids.len();
    LiveNode::Split {
        axis: SplitAxis::Horizontal,
        children: ids.iter().map(|i| leaf(*i)).collect(),
        sizes: vec![1.0 / n as f32; n],
    }
}

fn sizes(node: &LiveNode) -> Vec<f32> {
    match node {
        LiveNode::Split { sizes, .. } => sizes.clone(),
        LiveNode::Leaf(_) => Vec::new(),
    }
}

fn add_beside(tree: &mut LiveNode, target: u64, new: u64) {
    let pair = LiveNode::pair(SplitAxis::Horizontal, leaf(target), leaf(new));
    assert!(tree.replace_leaf(PaneId(target), pair));
}

#[test]
fn same_axis_insert_flattens_and_halves_slot() {
    let mut tree = LiveNode::pair(SplitAxis::Horizontal, leaf(1), leaf(2));
    add_beside(&mut tree, 2, 3);
    add_beside(&mut tree, 3, 4);
    let LiveNode::Split { children, .. } = &tree else {
        panic!("split");
    };
    assert_eq!(children.len(), 4);
    assert!(children.iter().all(|c| matches!(c, LiveNode::Leaf(_))));
    assert_eq!(sizes(&tree), vec![0.5, 0.25, 0.125, 0.125]);
}

#[test]
fn drag_changes_only_adjacent_sizes() {
    let mut tree = row(&[1, 2, 3, 4]);
    assert!(tree.set_boundary_for_split(PaneId(3), 0.6));
    let s = sizes(&tree);
    assert!((s[0] - 0.25).abs() < 1e-6 && (s[1] - 0.25).abs() < 1e-6);
    assert!((s[2] - 0.1).abs() < 1e-6 && (s[3] - 0.4).abs() < 1e-6);
    assert!(!tree.set_boundary_for_split(PaneId(4), 0.5));
}

#[test]
fn closing_a_pane_collapses_single_child_split() {
    let mut content = LiveContent {
        root: Some(row(&[1, 2])),
        focused: Some(PaneId(1)),
    };
    content.unsplit_empty(PaneId(1));
    assert!(matches!(content.root, Some(LiveNode::Leaf(ref l)) if l.id == PaneId(2)));
    assert_eq!(content.focused, Some(PaneId(2)));

    let mut content = LiveContent {
        root: Some(row(&[1, 2, 3])),
        focused: None,
    };
    content.unsplit_empty(PaneId(2));
    let root = content.root.unwrap();
    assert_eq!(root.leaf_ids(), vec![PaneId(1), PaneId(3)]);
    assert!((sizes(&root)[0] - 2.0 / 3.0).abs() < 1e-5);
}
