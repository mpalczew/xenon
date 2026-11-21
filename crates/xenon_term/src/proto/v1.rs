use serde::{Deserialize, Serialize};

use crate::{Size, TermStatus};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub(crate) enum ToHolder {
    View {
        #[serde(default)]
        answers: bool,
    },
    Claim {
        size: Size,
        #[serde(default)]
        label: Option<String>,
    },
    Seen,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub(crate) enum FromHolder {
    Status(TermStatus),
    Size(Size),
    Screen,
}
