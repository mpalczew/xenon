use super::*;
use crate::diff_text::{Kind, Line};

type Column = fn(&Line) -> u32;

fn rgba(hex: &str, alpha: f32) -> gdk::RGBA {
    let c = gdk::RGBA::parse(hex).unwrap_or(gdk::RGBA::new(0.5, 0.5, 0.5, 1.0));
    gdk::RGBA::new(c.red(), c.green(), c.blue(), alpha)
}

fn rgba_of(c: gdk::RGBA, alpha: f32) -> gdk::RGBA {
    gdk::RGBA::new(c.red(), c.green(), c.blue(), alpha)
}

fn set_text(label: &gtk::Label, text: Option<&str>) {
    label.set_visible(text.is_some());
    label.set_text(text.unwrap_or_default());
}

fn mark_lines(buf: &sourceview::Buffer, lines: &[Line], words: &[(&str, gdk::RGBA); 2]) {
    for (at, line) in lines.iter().enumerate() {
        let category = match line.kind {
            Kind::Added => "added",
            Kind::Removed => "removed",
            Kind::Filler | Kind::Header => "filler",
            Kind::Context => continue,
        };
        if let Some(iter) = buf.iter_at_line(at as i32) {
            buf.create_source_mark(None, category, &iter);
        }
        for &(start, end) in &line.words {
            let name = words[usize::from(line.kind == Kind::Removed)].0;
            if buf.tag_table().lookup(name).is_none() {
                let bg = &words[usize::from(line.kind == Kind::Removed)].1;
                buf.create_tag(Some(name), &[("background-rgba", bg)]);
            }
            if let (Some(a), Some(b)) = (
                buf.iter_at_line_offset(at as i32, start),
                buf.iter_at_line_offset(at as i32, end),
            ) {
                buf.apply_tag_by_name(name, &a, &b);
            }
        }
    }
}

fn set_marks(view: &sourceview::View, tints: &[(&str, gdk::RGBA)]) {
    for (name, bg) in tints {
        let attrs = sourceview::MarkAttributes::new();
        attrs.set_background(bg);
        if *name == "current" {
            attrs.set_icon_name("go-next-symbolic");
        }
        view.set_mark_attributes(name, &attrs, i32::from(*name == "current") * 10);
    }
}

impl DiffView {
    pub(super) fn header(&self, doc: &Doc, layout: Layout) {
        self.sub_label.set_text(&doc.subtitle);
        self.plus.set_text(&format!("+{}", doc.added));
        self.minus.set_text(&format!("-{}", doc.removed));
        self.auto_label
            .set_visible(self.mode.get() == LayoutMode::Auto);
        self.auto_label.set_text(if layout == Layout::Split {
            "\u{2192} split"
        } else {
            "\u{2192} unified"
        });
        self.banner
            .set_title(doc.banner.as_deref().unwrap_or_default());
        self.banner.set_revealed(doc.banner.is_some());
        set_text(&self.note, doc.note.as_deref());
    }

    pub(super) fn fill(&self, page: &Page) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        self.views.borrow_mut().clear();
        let accent = adw::StyleManager::default().accent_color_rgba();
        let (green, red) = ("#26a269", "#e01b24");
        let tints = [
            ("added", rgba(green, 0.14)),
            ("removed", rgba(red, 0.14)),
            ("filler", rgba("#808080", 0.07)),
            ("current", rgba_of(accent, 0.26)),
        ];
        let words = [("w-add", rgba(green, 0.35)), ("w-del", rgba(red, 0.35))];
        let scheme = crate::theme::scheme(crate::theme::dark());
        let lang = sourceview::LanguageManager::default()
            .guess_language(Some(std::path::Path::new(&self.path)), None::<&str>);
        let split = page.sides.len() > 1;
        let columns: [&[Column]; 2] = if split {
            [&[|l| l.old], &[|l| l.new]]
        } else {
            [&[|l| l.old, |l| l.new], &[]]
        };
        for (i, lines) in page.sides.iter().enumerate() {
            let buf = sourceview::Buffer::new(None);
            let text: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
            buf.set_text(&text.join("\n"));
            buf.set_highlight_syntax(true);
            buf.set_language(lang.as_ref());
            buf.set_style_scheme(scheme.as_ref());
            let view = sourceview::View::with_buffer(&buf);
            view.add_css_class("xdiff-code");
            view.set_editable(false);
            view.set_cursor_visible(false);
            view.set_monospace(true);
            view.set_wrap_mode(gtk::WrapMode::None);
            view.set_show_line_marks(true);
            view.set_top_margin(8);
            view.set_bottom_margin(12);
            view.set_left_margin(6);
            view.set_hexpand(true);
            set_marks(&view, &tints);
            let gutter = sourceview::prelude::ViewExt::gutter(&view, gtk::TextWindowType::Left);
            for (n, number) in columns[i].iter().enumerate() {
                let digits = lines
                    .iter()
                    .map(number)
                    .max()
                    .unwrap_or(0)
                    .to_string()
                    .len();
                let (w, _) = view
                    .create_pango_layout(Some(&"0".repeat(digits.max(3))))
                    .pixel_size();
                let numbers: Vec<u32> = lines.iter().map(number).collect();
                let column = sourceview::GutterRendererText::new();
                column.set_xalign(1.0);
                column.set_xpad(6);
                column.set_width_request(w + 12);
                column.add_css_class("dim-label");
                // sync: the renderer draws the text as soon as the handler returns
                column.connect_query_data(move |column, _, line| {
                    let n = numbers.get(line as usize).copied().unwrap_or(0);
                    column.set_text(&if n == 0 { String::new() } else { n.to_string() });
                });
                gutter.insert(&column, n as i32);
            }
            mark_lines(&buf, lines, &words);
            let pane = gtk::ScrolledWindow::new();
            pane.set_child(Some(&view));
            pane.set_hexpand(true);
            pane.set_vadjustment(Some(&self.vadj));
            pane.set_vscrollbar_policy(if i + 1 == page.sides.len() {
                gtk::PolicyType::Automatic
            } else {
                gtk::PolicyType::External
            });
            if i == 1 {
                pane.add_css_class("xdiff-half");
                pane.add_css_class("right");
            }
            self.body.append(&pane);
            self.views.borrow_mut().push(view);
        }
        self.body.set_homogeneous(split);
        if let Some(view) = self.views.borrow().first() {
            let h = view.iter_location(&view.buffer().start_iter()).height();
            if h > 4 {
                self.line_h.set(f64::from(h));
            }
        }
    }

    pub(super) fn mark_current(&self) {
        let line = self.hunks.borrow().get(self.current.get()).map(|h| h.0);
        for view in self.views.borrow().iter() {
            let buf = view
                .buffer()
                .downcast::<sourceview::Buffer>()
                .expect("source buffer");
            buf.remove_source_marks(&buf.start_iter(), &buf.end_iter(), Some("current"));
            if let Some(iter) = line.and_then(|l| buf.iter_at_line(l)) {
                buf.create_source_mark(None, "current", &iter);
            }
        }
    }
}
