use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{StoreError, data_dir, read_json, write_atomic};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProseMarkers {
    #[default]
    Dim,
    Hide,
}

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
}

fn default_size() -> f32 {
    18.0
}
fn default_line_height() -> f32 {
    1.55
}

impl Default for ProseSettings {
    fn default() -> Self {
        Self {
            markers: ProseMarkers::Dim,
            font: ProseFont::Auto,
            size: default_size(),
            line_height: default_line_height(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkdownMode {
    #[default]
    Prose,
    Split,
    Source,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProseFilePrefs {
    #[serde(default)]
    pub mode: MarkdownMode,
    #[serde(default = "yes")]
    pub paper: bool,
    /// Leading `<!-- -->` lines shown instead of folded into the chip.
    #[serde(default)]
    pub comments_open: bool,
    #[serde(default = "yes")]
    pub list: bool,
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
            list: true,
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
