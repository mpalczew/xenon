pub fn char_col_to_utf16(line: &str, char_col: usize) -> u32 {
    line.chars()
        .take(char_col)
        .map(char::len_utf16)
        .sum::<usize>() as u32
}

pub fn utf16_to_char_col(line: &str, utf16_col: u32) -> usize {
    let mut units = 0_u32;
    for (index, ch) in line.chars().enumerate() {
        let next = units + ch.len_utf16() as u32;
        if next > utf16_col {
            return index;
        }
        units = next;
    }
    line.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_non_bmp_columns() {
        let line = "a😀b";
        assert_eq!(char_col_to_utf16(line, 0), 0);
        assert_eq!(char_col_to_utf16(line, 2), 3);
        assert_eq!(utf16_to_char_col(line, 3), 2);
        assert_eq!(utf16_to_char_col(line, 2), 1);
    }
}
