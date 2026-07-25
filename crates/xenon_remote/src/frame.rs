//! Pure viewport text assembly (no scrollback).

/// Build monospaced lines for the **visible** grid only.
///
/// `cells` are `(display_row, col, char)` already adjusted with display_offset
/// so row `0` is the top of the viewport. Cells outside `0..rows` / `0..cols`
/// are ignored — history above the viewport never appears.
pub fn viewport_lines(
    cells: impl IntoIterator<Item = (i32, usize, char)>,
    cols: u16,
    rows: u16,
) -> Vec<String> {
    let cols_n = cols as usize;
    let rows_n = rows as usize;
    if cols_n == 0 || rows_n == 0 {
        return Vec::new();
    }
    let mut grid = vec![vec![' '; cols_n]; rows_n];
    for (row, col, ch) in cells {
        if row < 0 || col >= cols_n {
            continue;
        }
        let r = row as usize;
        if r >= rows_n {
            continue;
        }
        grid[r][col] = ch;
    }
    grid.into_iter()
        .map(|row| {
            let s: String = row.into_iter().collect();
            s.trim_end().to_string()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_negative_rows_scrollback() {
        // display_row -2 is scrollback; must not appear.
        let lines = viewport_lines([(-2, 0, 'H'), (0, 0, 'a'), (0, 1, 'b'), (1, 0, 'c')], 4, 2);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "ab");
        assert_eq!(lines[1], "c");
        assert!(!lines.iter().any(|l| l.contains('H')));
    }

    #[test]
    fn clamps_to_viewport_dims() {
        let lines = viewport_lines([(0, 0, 'x'), (0, 99, 'y'), (50, 0, 'z')], 2, 1);
        assert_eq!(lines, vec!["x".to_string()]);
    }

    #[test]
    fn empty_dims_yield_no_lines() {
        assert!(viewport_lines([(0, 0, 'a')], 0, 5).is_empty());
        assert!(viewport_lines([(0, 0, 'a')], 5, 0).is_empty());
    }
}
