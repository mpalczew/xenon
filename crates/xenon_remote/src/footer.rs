//! The sidebar footer's states and wording.

/// What the sidebar footer shows while the remote is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteFooter {
    /// On, but the other Xenon serves the phone for now.
    Waiting,
    Listening,
    Connected {
        device: String,
    },
    Driving {
        device: String,
        workspace: String,
    },
}

impl RemoteFooter {
    /// Footer state from the facts. `devices` are the short names of the
    /// connected phones; `driving` is the device and workspace of a phone
    /// whose grid is in effect.
    pub fn from_facts(
        waiting: bool,
        devices: &[String],
        driving: Option<(String, String)>,
    ) -> Self {
        if waiting {
            return Self::Waiting;
        }
        if let Some((device, workspace)) = driving {
            return Self::Driving { device, workspace };
        }
        match devices {
            [] => Self::Listening,
            [device] => Self::Connected {
                device: device.clone(),
            },
            more => Self::Connected {
                device: format!("{} phones", more.len()),
            },
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Waiting => "Phone remote · other Xenon has it".into(),
            Self::Listening => "Phone remote · listening".into(),
            Self::Connected { device } => format!("{device} connected"),
            Self::Driving { device, workspace } => format!("{device} driving · {workspace}"),
        }
    }

    /// A phone is connected (the accent state).
    pub fn active(&self) -> bool {
        matches!(self, Self::Connected { .. } | Self::Driving { .. })
    }
}
