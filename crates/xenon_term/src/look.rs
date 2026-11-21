use xenon_core::terminal_palette;
use xenon_remote::Rgb;

fn parse(hex: &str) -> Rgb {
    let byte = |i: usize| {
        hex.get(i..i + 2)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .unwrap_or(0)
    };
    Rgb(byte(1), byte(3), byte(5))
}

pub struct Look {
    pub fg: Rgb,
    pub bg: Rgb,
    pub ansi: [Rgb; 16],
}

impl Look {
    pub fn of(dark: bool) -> Self {
        let (fg, bg, ansi) = terminal_palette(dark);
        Self {
            fg: parse(fg),
            bg: parse(bg),
            ansi: ansi.map(parse),
        }
    }

    pub fn indexed(&self, index: u8) -> Rgb {
        match index {
            0..=15 => self.ansi[usize::from(index)],
            16..=231 => {
                let n = index - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
                Rgb(level(n / 36), level(n / 6 % 6), level(n % 6))
            }
            _ => {
                let v = 8 + 10 * (index - 232);
                Rgb(v, v, v)
            }
        }
    }

    pub fn theme(&self) -> [[u8; 3]; 3] {
        let bytes = |c: Rgb| [c.0, c.1, c.2];
        [bytes(self.bg), bytes(self.fg), bytes(self.fg)]
    }
}

pub fn dim(fg: Rgb, bg: Rgb) -> Rgb {
    let mix = |a: u8, b: u8| ((u16::from(a) * 2 + u16::from(b)) / 3) as u8;
    Rgb(mix(fg.0, bg.0), mix(fg.1, bg.1), mix(fg.2, bg.2))
}
