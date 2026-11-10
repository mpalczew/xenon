use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{StoreError, data_dir, load_or_default, write_atomic};

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
#[serde(default)]
pub struct ProseSettings {
    pub markers: ProseMarkers,
    pub font: ProseFont,
    pub size: f32,
    pub line_height: f32,
}

impl Default for ProseSettings {
    fn default() -> Self {
        Self {
            markers: ProseMarkers::Dim,
            font: ProseFont::Auto,
            size: 18.0,
            line_height: 1.55,
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
#[serde(default)]
pub struct ProseFilePrefs {
    pub mode: MarkdownMode,
    pub paper: bool,
    /// Leading `<!-- -->` lines shown instead of folded into the chip.
    pub comments_open: bool,
    pub list: bool,
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
#[serde(default)]
struct ProseFiles {
    files: BTreeMap<String, ProseFilePrefs>,
}

fn files_path() -> std::path::PathBuf {
    data_dir().join("prose-files.json")
}

fn load() -> ProseFiles {
    load_or_default(&files_path()).unwrap_or_default()
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
