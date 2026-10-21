use super::*;

fn leaf(id: u64) -> LiveNode {
    LiveNode::Leaf(LiveLeaf {
        id: PaneId(id),
        tabs: Vec::new(),
        active: 0,
        parked: false,
    })
}

fn split(first: LiveNode, second: LiveNode) -> LiveNode {
    LiveNode::Split {
        axis: SplitAxis::Horizontal,
        ratio: 0.5,
        first: Box::new(first),
        second: Box::new(second),
    }
}

fn ratios(node: &LiveNode, out: &mut Vec<f32>) {
    if let LiveNode::Split {
        ratio,
        first,
        second,
        ..
    } = node
    {
        ratios(first, out);
        out.push(*ratio);
        ratios(second, out);
    }
}

/// Nested splits share a first leaf; each divider must still map to its own split.
#[test]
fn set_ratio_targets_each_nested_divider() {
    let mut tree = split(split(split(leaf(1), leaf(2)), leaf(3)), leaf(4));
    for (key, expect) in [
        (1, [0.3, 0.5, 0.5]),
        (2, [0.3, 0.3, 0.5]),
        (3, [0.3, 0.3, 0.3]),
    ] {
        assert!(tree.set_ratio_for_split(PaneId(key), 0.3));
        let mut got = Vec::new();
        ratios(&tree, &mut got);
        assert_eq!(got, expect);
    }
    assert!(!tree.set_ratio_for_split(PaneId(4), 0.3));
}
