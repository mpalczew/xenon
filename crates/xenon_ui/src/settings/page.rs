//! The Settings sidebar pages, in ⌘1–⌘7 order.

use lucide_icons::Icon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsPage {
    Appearance,
    Fonts,
    Editor,
    Terminal,
    Agents,
    PhoneRemote,
    LanguageServers,
}

impl SettingsPage {
    pub const ALL: [Self; 7] = [
        Self::Appearance,
        Self::Fonts,
        Self::Editor,
        Self::Terminal,
        Self::Agents,
        Self::PhoneRemote,
        Self::LanguageServers,
    ];

    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Fonts => "Fonts",
            Self::Editor => "Editor",
            Self::Terminal => "Terminal",
            Self::Agents => "Agents",
            Self::PhoneRemote => "Phone Remote",
            Self::LanguageServers => "Language Servers",
        }
    }

    pub(super) fn icon(self) -> Icon {
        match self {
            Self::Appearance => Icon::SunMoon,
            Self::Fonts => Icon::Type,
            Self::Editor => Icon::FileCode,
            Self::Terminal => Icon::SquareTerminal,
            Self::Agents => Icon::Sparkles,
            Self::PhoneRemote => Icon::Smartphone,
            Self::LanguageServers => Icon::Braces,
        }
    }

    /// Pages whose settings change what the live preview shows.
    pub(super) fn has_preview(self) -> bool {
        matches!(self, Self::Appearance | Self::Fonts | Self::Editor)
    }

    /// ⌘1 → first page.
    pub(super) fn from_digit(key: &str) -> Option<Self> {
        let index = key.parse::<usize>().ok()?.checked_sub(1)?;
        Self::ALL.get(index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsPage;

    #[test]
    fn digits_map_to_pages() {
        assert_eq!(
            SettingsPage::from_digit("1"),
            Some(SettingsPage::Appearance)
        );
        assert_eq!(
            SettingsPage::from_digit("7"),
            Some(SettingsPage::LanguageServers)
        );
        assert_eq!(SettingsPage::from_digit("0"), None);
        assert_eq!(SettingsPage::from_digit("8"), None);
    }
}
