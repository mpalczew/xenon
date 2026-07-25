//! Pure visible-grid assembly (no scrollback). Shared algorithm with
//! `xenon_remote::viewport_lines` — keep behaviour aligned.

/// Build monospaced lines for the **visible** grid only.
///
/// `cells` are `(display_row, col, char)` with row `0` as the top of the
/// viewport. Cells outside `0..rows` / `0..cols` are ignored.
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
    fn ignores_scrollback_rows() {
        let lines = viewport_lines([(-1, 0, 'H'), (0, 0, 'a'), (0, 1, 'b'), (1, 0, 'c')], 4, 2);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "ab");
        assert_eq!(lines[1], "c");
        assert!(!lines.iter().any(|l| l.contains('H')));
    }

    #[test]
    fn dims_match_viewport_not_cell_count() {
        // Many cells out of bounds must not grow lines past `rows`.
        let cells: Vec<_> = (0..100)
            .flat_map(|r| (0..100).map(move |c| (r, c, 'x')))
            .collect();
        let lines = viewport_lines(cells, 3, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.chars().count() <= 3));
    }
}
