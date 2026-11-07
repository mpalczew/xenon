//! Prose mode look: the Markdown writing settings, the column and the tag sizes.

use super::*;

/// The font and size rule for a prose view, added to the window sheet.
pub(crate) fn css(prose: &ProseSettings, mono: &str) -> String {
    let family = match prose.font {
        ProseFont::Auto | ProseFont::Serif => "serif".to_string(),
        ProseFont::Sans => "sans-serif".to_string(),
        ProseFont::Mono => format!("\"{mono}\", monospace"),
    };
    format!(
        "textview.xeditor.xprose {{ font-family: {family}; font-size: {}px; }}\n",
        prose.size
    )
}

impl Prose {
    /// Apply the Markdown writing settings and the theme.
    pub(crate) fn restyle(self: &Rc<Self>, app: &App, fresh: bool) {
        let settings = app.settings.borrow();
        let p = settings.prose.clone();
        drop(settings);
        let hide = p.markers == ProseMarkers::Hide;
        let hide_changed = self.hide.replace(hide) != hide;
        self.view.add_css_class("xprose");
        self.view.set_monospace(false);
        self.view.set_show_line_numbers(false);
        self.view.set_highlight_current_line(false);
        self.buffer.set_highlight_syntax(false);
        let size = f64::from(p.size);
        let extra = (size * (f64::from(p.line_height) - 1.2)).max(0.0) as i32;
        self.view.set_pixels_inside_wrap(extra);
        self.view.set_pixels_above_lines(extra / 2);
        self.view.set_pixels_below_lines(extra - extra / 2);
        self.view.set_top_margin(28);
        self.view.set_bottom_margin(60);
        let margin = self.column_margin(&p);
        let look = self.look(app, &p, margin);
        self.tags.style(&app.palette.borrow(), &look);
        self.margin.set(margin);
        self.view.set_left_margin(margin);
        self.view.set_right_margin(margin);
        if fresh {
            self.edits.set(self.edits.get() + 1);
            self.arm(Duration::ZERO);
        } else if hide_changed {
            self.reapply();
        }
        self.images.schedule_move(self);
    }

    /// Run the stored document over the buffer again (a setting changed).
    pub(super) fn reapply(self: &Rc<Self>) {
        let generation = self.applies.get() + 1;
        self.applies.set(generation);
        let block = self
            .hide
            .get()
            .then(|| self.doc.borrow().block_at(self.caret()))
            .flatten();
        self.revealed.set(block);
        self.slice(generation, 0, 0, Vec::new());
    }

    /// Width of `text` in the prose font at the prose size.
    pub(super) fn text_width(&self, p: &ProseSettings, text: &str) -> i32 {
        let layout = pango::Layout::new(&self.view.pango_context());
        let mut font = pango::FontDescription::new();
        let family = match p.font {
            ProseFont::Auto | ProseFont::Serif => "Serif".to_string(),
            ProseFont::Sans => "Sans".to_string(),
            ProseFont::Mono => self
                .app
                .upgrade()
                .map(|a| a.mono.borrow().clone())
                .unwrap_or_default(),
        };
        font.set_family(&family);
        font.set_absolute_size(f64::from(p.size) * f64::from(pango::SCALE));
        layout.set_font_description(Some(&font));
        layout.set_text(text);
        layout.pixel_size().0.max(1)
    }

    /// Side margin that centers a column of `measure` characters.
    pub(super) fn column_margin(&self, p: &ProseSettings) -> i32 {
        let ch = self.text_width(p, "0");
        let column = (p.measure * ch as f32) as i32;
        ((self.scroller.width() - column) / 2).max(24)
    }

    /// The tag measurements for the current settings and column.
    pub(super) fn look(&self, app: &App, p: &ProseSettings, margin: i32) -> Look {
        let extra = (f64::from(p.size) * (f64::from(p.line_height) - 1.2)).max(0.0) as i32;
        Look {
            margin,
            gap: extra,
            hang: self.text_width(p, "- "),
            mono: app.mono.borrow().clone(),
        }
    }

    /// Re-center the column after the window changed width.
    pub(super) fn fit_column(self: &Rc<Self>) {
        if !self.on.get() {
            return;
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let p = app.settings.borrow().prose.clone();
        let margin = self.column_margin(&p);
        if (margin - self.margin.get()).abs() < 2 {
            return;
        }
        self.margin.set(margin);
        self.view.set_left_margin(margin);
        self.view.set_right_margin(margin);
        let look = self.look(&app, &p, margin);
        self.tags.style(&app.palette.borrow(), &look);
        self.images.schedule_move(self);
    }
}
