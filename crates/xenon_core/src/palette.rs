/// Terminal foreground, background and the 16 ANSI colors for light or dark.
/// Both come from the default "GNOME" palette of Ptyxis (GPL-3.0-or-later),
/// its Light and Dark variants, with that palette's own foreground and
/// background. The window and the phone page both read it from here.
pub fn terminal_palette(dark: bool) -> (&'static str, &'static str, [&'static str; 16]) {
    const LIGHT: [&str; 16] = [
        "#1d1d20", "#c01c28", "#26a269", "#a2734c", "#12488b", "#a347ba", "#2aa1b3", "#767676",
        "#5d5d5d", "#f66151", "#29a963", "#bd8d0a", "#2a7bde", "#c061cb", "#2aa2b5", "#949494",
    ];
    const DARK: [&str; 16] = [
        "#696672", "#c42934", "#2ec27e", "#f5c211", "#1e78e4", "#9841bb", "#0ab9dc", "#c0bfbc",
        "#848389", "#ed333b", "#57e389", "#f8e45c", "#51a1ff", "#c061cb", "#4fd2fd", "#f6f5f4",
    ];
    if dark {
        ("#ffffff", "#1c1c1f", DARK)
    } else {
        ("#1d1d20", "#ffffff", LIGHT)
    }
}
