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
