//! Overlay focus restore and window-deferred UI work.
//!
//! Palettes open without a `Window` in some subscription paths; focus restore
//! and command/workspace jumps queue here and drain during `Render`.

use super::*;

/// Which pane held keyboard focus before an overlay opened.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FocusPane {
    Terminal,
    Editor,
    Browser,
    Shell,
}

/// Content surface for cmd-+ / cmd-- font zoom (editor and terminal only).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum FontPane {
    #[default]
    Terminal,
    Editor,
}

/// Queued focus + palette/command work that needs a `Window` on the next paint.
#[derive(Default)]
pub(super) struct DeferredUi {
    /// Pane focused when the finder/palette opened.
    pub restore_pane: Option<FocusPane>,
    /// Re-focus this pane during the next render.
    pub pending_focus: Option<FocusPane>,
    /// Last editor/terminal focus for zoom when chrome has focus.
    pub last_font_pane: FontPane,
    /// cmd-clicked basename to open in the file palette.
    pub pending_palette_query: Option<String>,
    /// Command palette selection deferred until render.
    pub pending_command: Option<crate::commands::CommandId>,
    /// Workspace jump deferred until render.
    pub pending_workspace: Option<WorkspaceId>,
}
