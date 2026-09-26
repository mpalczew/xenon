//! Semantic text roles shared by Xenon's native surfaces.

use gpui::{App, FontWeight, Styled, px};
use theme::{ActiveTheme, ThemeColors};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeRole {
    ScreenTitle,
    SectionTitle,
    Body,
    ListPrimary,
    Supporting,
    ControlLabel,
    Button,
    Code,
}

impl TypeRole {
    fn metrics(self) -> (f32, f32, FontWeight) {
        match self {
            Self::ScreenTitle => (24., 1.2, FontWeight::SEMIBOLD),
            Self::SectionTitle => (18., 1.35, FontWeight::SEMIBOLD),
            Self::Body => (14., 1.5, FontWeight::NORMAL),
            Self::ListPrimary => (14., 1.35, FontWeight::SEMIBOLD),
            Self::Supporting => (14., 1.6, FontWeight::NORMAL),
            Self::ControlLabel | Self::Button => (12., 1.35, FontWeight::SEMIBOLD),
            Self::Code => (13., 1.55, FontWeight::NORMAL),
        }
    }

    fn color(self, colors: &ThemeColors) -> gpui::Hsla {
        match self {
            Self::Supporting | Self::ControlLabel => colors.text_muted,
            _ => colors.text,
        }
    }
}

pub trait Typography: Styled + Sized {
    fn type_role(self, role: TypeRole, cx: &App) -> Self {
        let (size, line_height, weight) = role.metrics();
        let ui = xenon_settings::ui_font(cx);
        let scaled = size * ui.size / 14.;
        let text = self
            .text_size(px(scaled))
            .line_height(px(scaled * line_height))
            .font_weight(weight)
            .text_color(role.color(cx.theme().colors()));
        match role {
            TypeRole::Code => text.font_family(xenon_settings::editor_font(cx).family),
            _ => text.font_family(ui.family),
        }
    }
}

impl<T: Styled> Typography for T {}
