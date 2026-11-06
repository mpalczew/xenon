//! Bundled faces, as raw bytes for a frontend to register with its text system.
//!
//! Lilex and IBM Plex Sans: see ATTRIBUTION.md (OFL-1.1, via zed assets).

/// Every bundled face file (Lilex, IBM Plex Sans).
///
/// Linux maps `.SystemUIFont` to IBM Plex Sans. Without these files the
/// chrome falls through the fallback stack onto Lilex.
pub const EMBEDDED_FONTS: [&[u8]; 8] = [
    include_bytes!("../fonts/lilex/Lilex-Regular.ttf"),
    include_bytes!("../fonts/lilex/Lilex-Bold.ttf"),
    include_bytes!("../fonts/lilex/Lilex-Italic.ttf"),
    include_bytes!("../fonts/lilex/Lilex-BoldItalic.ttf"),
    include_bytes!("../fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"),
    include_bytes!("../fonts/ibm-plex-sans/IBMPlexSans-Italic.ttf"),
    include_bytes!("../fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf"),
    include_bytes!("../fonts/ibm-plex-sans/IBMPlexSans-SemiBoldItalic.ttf"),
];
