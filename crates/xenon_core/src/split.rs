//! Shared N-ary split invariants for saved and live pane trees.

use crate::content::{MIN_SIZE, PaneNode, SplitAxis};

/// A tree whose splits can be flattened by [`flatten_split`].
pub trait SplitTree: Sized {
    /// `self`'s children and sizes when it is a split on `axis`; else `self`.
    fn into_axis_parts(self, axis: SplitAxis) -> Result<(Vec<Self>, Vec<f32>), Self>;
}

impl SplitTree for PaneNode {
    fn into_axis_parts(self, axis: SplitAxis) -> Result<(Vec<Self>, Vec<f32>), Self> {
        match self {
            Self::Split {
                axis: a,
                children,
                sizes,
            } if a == axis => Ok((children, sizes)),
            other => Err(other),
        }
    }
}

/// Enforce the split invariants. Splices same-axis child splits (scaling their
/// sizes by their slot), renormalizes and clamps sizes. `Err(only)` when one
/// child remains and the split should collapse to it; `Err` is never empty
/// unless `children` was.
pub fn flatten_split<T: SplitTree>(
    axis: SplitAxis,
    children: Vec<T>,
    mut sizes: Vec<f32>,
) -> Result<(Vec<T>, Vec<f32>), Option<T>> {
    normalize_sizes(&mut sizes, children.len());
    let mut out_children = Vec::with_capacity(children.len());
    let mut out_sizes = Vec::with_capacity(children.len());
    for (child, slot) in children.into_iter().zip(sizes) {
        match child.into_axis_parts(axis) {
            Ok((inner, mut inner_sizes)) => {
                normalize_sizes(&mut inner_sizes, inner.len());
                for (c, s) in inner.into_iter().zip(inner_sizes) {
                    out_children.push(c);
                    out_sizes.push(s * slot);
                }
            }
            Err(child) => {
                out_children.push(child);
                out_sizes.push(slot);
            }
        }
    }
    if out_children.len() <= 1 {
        return Err(out_children.pop());
    }
    normalize_sizes(&mut out_sizes, out_children.len());
    Ok((out_children, out_sizes))
}

/// Make `sizes` exactly `n` positive fractions summing to 1, each at least
/// `MIN_SIZE` (or `1/n` when that is smaller).
pub fn normalize_sizes(sizes: &mut Vec<f32>, n: usize) {
    if n == 0 {
        sizes.clear();
        return;
    }
    if sizes.len() != n || sizes.iter().any(|s| !s.is_finite() || *s <= 0.0) {
        *sizes = vec![1.0 / n as f32; n];
        return;
    }
    let min = MIN_SIZE.min(1.0 / n as f32);
    let total: f32 = sizes.iter().sum();
    for s in sizes.iter_mut() {
        *s = (*s / total).max(min);
    }
    let total: f32 = sizes.iter().sum();
    for s in sizes.iter_mut() {
        *s /= total;
    }
}

/// Drag boundary `i` (between children `i` and `i+1`) to fractional position
/// `pos` of the whole split. Only sizes `i` and `i+1` change; their sum is fixed.
pub fn move_boundary(sizes: &mut [f32], i: usize, pos: f32) -> bool {
    if i + 1 >= sizes.len() || !pos.is_finite() {
        return false;
    }
    let start: f32 = sizes[..i].iter().sum();
    let combined = sizes[i] + sizes[i + 1];
    let min = MIN_SIZE.min(combined / 2.0);
    let left = (pos - start).clamp(min, combined - min);
    sizes[i] = left;
    sizes[i + 1] = combined - left;
    true
}
