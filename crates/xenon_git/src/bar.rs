//! Five-cell change bar for the Git section.

/// One cell of a five-cell change bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarCell {
    Added,
    Removed,
    Neutral,
}

/// Five cells sized by the share of added and removed lines (at least one
/// cell for each side that changed).
pub fn change_bar(added: u32, removed: u32) -> [BarCell; 5] {
    let total = added + removed;
    let mut cells = [BarCell::Neutral; 5];
    if total == 0 {
        return cells;
    }
    let most = if added > 0 && removed > 0 { 4 } else { 5 };
    let share = |n: u32| {
        if n == 0 {
            0
        } else {
            ((5 * n + total / 2) / total).clamp(1, most) as usize
        }
    };
    let (mut green, mut red) = (share(added), share(removed));
    while green + red > 5 {
        if green >= red {
            green -= 1;
        } else {
            red -= 1;
        }
    }
    for (i, cell) in cells.iter_mut().enumerate() {
        if i < green {
            *cell = BarCell::Added;
        } else if i < green + red {
            *cell = BarCell::Removed;
        }
    }
    cells
}
