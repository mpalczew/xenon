//! Markdown prose-mode preferences: global look (`settings.json`) and the
//! per-file view state (`prose-files.json` in the data dir).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{StoreError, data_dir, read_json, write_atomic};

/// How inline Markdown syntax (`**`, `#`, `[`…) is drawn in prose mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProseMarkers {
    #[default]
    Dim,
    Hide,
}

/// Face for prose text. Auto is serif on Paper and the UI sans otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProseFont {
    #[default]
    Auto,
    Serif,
    Sans,
    Mono,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProseSettings {
    #[serde(default)]
    pub markers: ProseMarkers,
    #[serde(default)]
    pub font: ProseFont,
    #[serde(default = "default_size")]
    pub size: f32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    /// Column width in characters (ch).
    #[serde(default = "default_measure")]
    pub measure: f32,
}

fn default_size() -> f32 {
    18.0
}
fn default_line_height() -> f32 {
    1.55
}
fn default_measure() -> f32 {
    66.0
}

impl Default for ProseSettings {
    fn default() -> Self {
        Self {
            markers: ProseMarkers::Dim,
            font: ProseFont::Auto,
            size: default_size(),
            line_height: default_line_height(),
            measure: default_measure(),
        }
    }
}

/// The three ways to look at a Markdown file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkdownMode {
    #[default]
    Prose,
    Split,
    Source,
}

/// View state remembered for one Markdown file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProseFilePrefs {
    #[serde(default)]
    pub mode: MarkdownMode,
    #[serde(default = "yes")]
    pub paper: bool,
    /// Leading `<!-- -->` lines shown instead of folded into the chip.
    #[serde(default)]
    pub comments_open: bool,
}

fn yes() -> bool {
    true
}

impl Default for ProseFilePrefs {
    fn default() -> Self {
        Self {
            mode: MarkdownMode::Prose,
            paper: true,
            comments_open: false,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct ProseFiles {
    #[serde(default)]
    files: BTreeMap<String, ProseFilePrefs>,
}

fn files_path() -> std::path::PathBuf {
    data_dir().join("prose-files.json")
}

fn load() -> ProseFiles {
    let path = files_path();
    if !path.exists() {
        return ProseFiles::default();
    }
    read_json(&path).unwrap_or_default()
}

/// Remembered view state for `path`, or the defaults.
pub fn prose_file_prefs(path: &Path) -> ProseFilePrefs {
    load()
        .files
        .get(&path.display().to_string())
        .copied()
        .unwrap_or_default()
}

pub fn set_prose_file_prefs(path: &Path, prefs: ProseFilePrefs) -> Result<(), StoreError> {
    let mut file = load();
    file.files.insert(path.display().to_string(), prefs);
    write_atomic(&files_path(), &file)
}
