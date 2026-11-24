use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use anyhow::{Context, Error, Result};
use sha2::{Digest, Sha256};

/// Content fingerprint plus how many earlier entries share it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Mark {
    fingerprint: u64,
    nth: u32,
}

impl fmt::Display for Mark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}-{}", self.fingerprint, self.nth)
    }
}

impl FromStr for Mark {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        let (fingerprint, nth) = text.split_once('-').context("not a worklist key")?;
        Ok(Self {
            fingerprint: u64::from_str_radix(fingerprint, 16)?,
            nth: nth.parse()?,
        })
    }
}

/// Marks for texts in file order: SHA-256 prefix, numbered among identical texts.
pub(crate) fn number(texts: impl Iterator<Item = String>) -> Vec<Mark> {
    let mut seen: HashMap<u64, u32> = HashMap::new();
    texts
        .map(|text| {
            let digest = Sha256::digest(text.as_bytes());
            let fingerprint = u64::from_be_bytes(std::array::from_fn(|i| digest[i]));
            let count = seen.entry(fingerprint).or_default();
            *count += 1;
            Mark {
                fingerprint,
                nth: *count - 1,
            }
        })
        .collect()
}

/// Names one item by its content, not its position. Stable across edits to other items
/// and across checking or unchecking this one. Display/FromStr: `"{16 hex}-{n}"`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ItemKey(Mark);

/// Names a section heading by its title and occurrence.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SectionKey(Mark);

impl From<Mark> for ItemKey {
    fn from(mark: Mark) -> Self {
        Self(mark)
    }
}

impl From<Mark> for SectionKey {
    fn from(mark: Mark) -> Self {
        Self(mark)
    }
}

impl fmt::Display for ItemKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ItemKey {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        text.parse().map(Self)
    }
}

/// Where new items go. Display/FromStr: `"top"` | `"s-{16 hex}-{n}"`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Place {
    #[default]
    Top,
    Section(SectionKey),
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Top => f.write_str("top"),
            Self::Section(SectionKey(mark)) => write!(f, "s-{mark}"),
        }
    }
}

impl FromStr for Place {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        match text.strip_prefix("s-") {
            Some(mark) => Ok(Self::Section(SectionKey(mark.parse()?))),
            None if text == "top" => Ok(Self::Top),
            None => Err(anyhow::anyhow!("not a worklist place")),
        }
    }
}
