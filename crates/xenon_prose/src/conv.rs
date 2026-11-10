use std::ops::Range;

pub(crate) struct Conv<'a> {
    pub(crate) text: &'a str,
    ascii: bool,
    lines: Vec<(usize, usize)>,
}

impl<'a> Conv<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        let ascii = text.is_ascii();
        let mut lines = Vec::new();
        if !ascii {
            let (mut chars, mut from) = (0, 0);
            lines.push((0, 0));
            for (i, _) in text.match_indices('\n') {
                chars += text[from..=i].chars().count();
                from = i + 1;
                lines.push((from, chars));
            }
        }
        Self { text, ascii, lines }
    }

    pub(crate) fn at(&self, byte: usize) -> usize {
        let byte = byte.min(self.text.len());
        if self.ascii {
            return byte;
        }
        let i = self
            .lines
            .partition_point(|(b, _)| *b <= byte)
            .saturating_sub(1);
        let (line_byte, line_char) = self.lines[i];
        line_char + self.text[line_byte..byte].chars().count()
    }

    pub(crate) fn range(&self, r: &Range<usize>) -> Range<usize> {
        self.at(r.start)..self.at(r.end)
    }

    pub(crate) fn line_start(&self, byte: usize) -> usize {
        let byte = byte.min(self.text.len());
        self.text[..byte].rfind('\n').map_or(0, |i| i + 1)
    }

    pub(crate) fn line_end(&self, byte: usize) -> usize {
        let byte = byte.min(self.text.len());
        self.text[byte..]
            .find('\n')
            .map_or(self.text.len(), |i| byte + i)
    }

    pub(crate) fn trim_end(&self, r: &Range<usize>) -> Range<usize> {
        let mut end = r.end.min(self.text.len());
        while end > r.start && self.text.as_bytes()[end - 1] == b'\n' {
            end -= 1;
        }
        r.start..end
    }
}
