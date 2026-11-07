//! Painting the diff: theme colors, the header strip, filling the text views and
//! marking the current hunk. A child of `diff_view`, so it reaches the view's fields.

use super::*;

/// Colors for tags, from the theme.
pub(super) struct Tones {
    added: gdk::RGBA,
    removed: gdk::RGBA,
    word_added: gdk::RGBA,
    word_removed: gdk::RGBA,
    muted: gdk::RGBA,
    accent: gdk::RGBA,
    header_bg: gdk::RGBA,
    text: gdk::RGBA,
}

fn rgba(hex: &str, alpha: Option<f32>) -> gdk::RGBA {
    let c = gdk::RGBA::parse(hex).unwrap_or(gdk::RGBA::new(0.5, 0.5, 0.5, 1.0));
    match alpha {
        Some(a) => gdk::RGBA::new(c.red(), c.green(), c.blue(), a),
        None => c,
    }
}

impl DiffView {
    pub(super) fn header(&self, doc: &DiffDoc, layout: Layout) {
        self.sub_label.set_text(&doc.subtitle);
        self.plus.set_text(&format!("+{}", doc.added));
        self.minus.set_text(&format!("-{}", doc.removed));
        let auto = self.mode.get() == LayoutMode::Auto && self.scroll.width() > 0;
        self.auto_label.set_visible(auto);
        self.auto_label.set_text(&format!(
            "\u{2192} {} \u{b7} {}px",
            if layout == Layout::Split {
                "split"
            } else {
                "unified"
            },
            self.scroll.width()
        ));
        set_text(&self.banner, doc.banner.as_deref());
        let mut note = doc.note.clone().unwrap_or_default();
        if doc.truncated {
            if !note.is_empty() {
                note.push(' ');
            }
            note.push_str("Diff truncated; open the file to see the rest.");
        }
        set_text(&self.note, (!note.is_empty()).then_some(note.as_str()));
    }

    pub(super) fn tones(&self, app: &App) -> Tones {
        let p = app.palette.borrow();
        let c = |key: &str, fallback: &str| p.color(key, fallback);
        Tones {
            added: rgba(&c("version_control.added", "#27a657"), None),
            removed: rgba(&c("version_control.deleted", "#e06c76"), None),
            word_added: rgba(&c("version_control.word_added", "#2EA04859"), None),
            word_removed: rgba(&c("version_control.word_deleted", "#F8514980"), None),
            muted: rgba(&c("text.muted", "#808080"), None),
            accent: rgba(&c("text.accent", "#74ade8"), None),
            header_bg: rgba(&c("element.background", "#80808020"), None),
            text: rgba(&c("text", "#cccccc"), None),
        }
    }

    pub(super) fn fill(&self, app: &App, page: &Page) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        self.views.borrow_mut().clear();
        let tones = self.tones(app);
        let dark = adw::StyleManager::default().is_dark();
        let scheme = sourceview::StyleSchemeManager::default().scheme(if dark {
            "Adwaita-dark"
        } else {
            "Adwaita"
        });
        let lang = sourceview::LanguageManager::default()
            .guess_language(Some(std::path::Path::new(&self.path)), None::<&str>);
        for (i, side) in page.sides.iter().enumerate() {
            let code_buf = sourceview::Buffer::new(None);
            code_buf.set_text(&side.code);
            code_buf.set_highlight_syntax(true);
            code_buf.set_language(lang.as_ref());
            code_buf.set_style_scheme(scheme.as_ref());
            let gutter_buf = gtk::TextBuffer::new(None);
            gutter_buf.set_text(&side.gutter);
            paint(&code_buf, &gutter_buf, side, &tones);
            let gutter = text_view(gtk::TextView::with_buffer(&gutter_buf));
            gutter.add_css_class("xdiff-gutter");
            gutter.set_can_target(false);
            gutter.set_left_margin(8);
            gutter.set_right_margin(6);
            // A text view reports no width until its text is laid out; give
            // the gutter the width of its widest line up front.
            let chars = side
                .gutter
                .lines()
                .map(|l| l.chars().count())
                .max()
                .unwrap_or(1);
            let (w, _) = gutter
                .create_pango_layout(Some(&"0".repeat(chars)))
                .pixel_size();
            gutter.set_size_request(w + 16, -1);
            // GtkSource only colors text for a view of its own kind.
            let code = text_view(sourceview::View::with_buffer(&code_buf).upcast());
            code.add_css_class("xdiff-code");
            code.set_left_margin(6);
            code.set_hexpand(true);
            let half = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            half.add_css_class("xdiff-half");
            if i == 1 {
                half.add_css_class("right");
            }
            half.set_hexpand(true);
            half.set_homogeneous(false);
            half.append(&gutter);
            half.append(&code);
            self.body.append(&half);
            self.views.borrow_mut().push((gutter, code));
        }
        self.body.set_homogeneous(page.sides.len() > 1);
        if let Some((_, code)) = self.views.borrow().first() {
            let h = code.iter_location(&code.buffer().start_iter()).height();
            if h > 4 {
                self.line_h.set(f64::from(h));
            }
        }
    }

    /// Mark the current hunk's header lines.
    pub(super) fn mark_current(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let accent = app.palette.borrow().color("text.accent", "#74ade8");
        let line = self.hunks.borrow().get(self.current.get()).map(|h| h.0);
        for (gutter, code) in self.views.borrow().iter() {
            for view in [gutter, code] {
                let buf = view.buffer();
                let tag = buf.tag_table().lookup("t-current").unwrap_or_else(|| {
                    buf.create_tag(
                        Some("t-current"),
                        &[("paragraph-background-rgba", &rgba(&accent, Some(0.26)))],
                    )
                    .expect("fresh tag")
                });
                buf.remove_tag(&tag, &buf.start_iter(), &buf.end_iter());
                if let Some(line) = line
                    && let Some(start) = buf.iter_at_line(line)
                {
                    let end = buf.iter_at_line(line + 1).unwrap_or_else(|| buf.end_iter());
                    buf.apply_tag(&tag, &start, &end);
                }
            }
        }
    }
}

fn set_text(label: &gtk::Label, text: Option<&str>) {
    label.set_visible(text.is_some());
    label.set_text(text.unwrap_or_default());
}

fn text_view(view: gtk::TextView) -> gtk::TextView {
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_monospace(true);
    view.set_wrap_mode(gtk::WrapMode::None);
    view.set_focusable(false);
    view.set_hscroll_policy(gtk::ScrollablePolicy::Natural);
    view.set_vscroll_policy(gtk::ScrollablePolicy::Natural);
    view.set_top_margin(8);
    view.set_bottom_margin(12);
    view
}

/// Tint lines and changed words with text tags.
fn paint(code: &sourceview::Buffer, gutter: &gtk::TextBuffer, side: &Side, t: &Tones) {
    let tag = |buf: &gtk::TextBuffer, name: &str, props: &[(&str, &dyn ToValue)]| {
        buf.create_tag(Some(name), props).expect("fresh tag")
    };
    let tint = |c: &gdk::RGBA, a: f32| gdk::RGBA::new(c.red(), c.green(), c.blue(), a);
    for buf in [code.upcast_ref::<gtk::TextBuffer>(), gutter] {
        tag(
            buf,
            "t-add",
            &[("paragraph-background-rgba", &tint(&t.added, 0.14))],
        );
        tag(
            buf,
            "t-del",
            &[("paragraph-background-rgba", &tint(&t.removed, 0.14))],
        );
        tag(
            buf,
            "t-fill",
            &[("paragraph-background-rgba", &tint(&t.muted, 0.07))],
        );
        tag(
            buf,
            "t-head",
            &[
                ("paragraph-background-rgba", &t.header_bg),
                ("foreground-rgba", &t.accent),
            ],
        );
        tag(
            buf,
            "t-title",
            &[
                ("foreground-rgba", &t.text),
                ("weight", &700i32),
                ("paragraph-background-rgba", &tint(&t.muted, 0.07)),
            ],
        );
    }
    tag(gutter, "g-muted", &[("foreground-rgba", &t.muted)]);
    tag(gutter, "g-add", &[("foreground-rgba", &t.added)]);
    tag(gutter, "g-del", &[("foreground-rgba", &t.removed)]);
    tag(
        code.upcast_ref(),
        "w-add",
        &[("background-rgba", &t.word_added)],
    );
    tag(
        code.upcast_ref(),
        "w-del",
        &[("background-rgba", &t.word_removed)],
    );
    gutter.apply_tag_by_name("g-muted", &gutter.start_iter(), &gutter.end_iter());
    let line_range = |buf: &gtk::TextBuffer, line: i32| {
        let start = buf.iter_at_line(line)?;
        let end = buf.iter_at_line(line + 1).unwrap_or_else(|| buf.end_iter());
        Some((start, end))
    };
    for mark in &side.marks {
        let name = match mark.tint {
            Tint::Added => "t-add",
            Tint::Removed => "t-del",
            Tint::Filler => "t-fill",
            Tint::Header => "t-head",
            Tint::Title => "t-title",
        };
        for buf in [code.upcast_ref::<gtk::TextBuffer>(), gutter] {
            if let Some((a, b)) = line_range(buf, mark.line) {
                buf.apply_tag_by_name(name, &a, &b);
            }
        }
        // The sign at the end of the gutter text takes the line's color.
        let sign = match mark.tint {
            Tint::Added => Some("g-add"),
            Tint::Removed => Some("g-del"),
            _ => None,
        };
        if let (Some(sign), Some(mut end)) = (sign, gutter.iter_at_line(mark.line)) {
            end.forward_to_line_end();
            let mut start = end;
            if start.backward_char() {
                gutter.apply_tag_by_name(sign, &start, &end);
            }
        }
    }
    for word in &side.words {
        if let (Some(mut a), Some(mut b)) = (
            code.iter_at_line_offset(word.line, word.start),
            code.iter_at_line_offset(word.line, word.end),
        ) {
            if b.line() != word.line {
                b.backward_char();
            }
            let _ = &mut a;
            code.apply_tag_by_name(if word.added { "w-add" } else { "w-del" }, &a, &b);
        }
    }
}
