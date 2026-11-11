use std::f64::consts::TAU;

use gtk::cairo;

use super::*;

impl Prose {
    pub(super) fn sync_decor(self: &Rc<Self>) {
        if self.decor_pending.replace(true) {
            return;
        }
        let this = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(this) = this.upgrade() else { return };
            this.decor_pending.set(false);
            let height = this.scroller.vadjustment().upper() as i32;
            this.decor
                .set_size_request(this.scroller.width().max(1), height);
            this.decor.queue_draw();
        });
    }

    pub(super) fn draw_decor(&self, cr: &cairo::Context) {
        let view = &self.view;
        let vis = view.visible_rect();
        let (top, bottom) = (vis.y(), vis.y() + vis.height());
        let total = self.buffer.char_count() as usize;
        let at = |off: usize| self.at(off.min(total));
        let paint = |c: gdk::RGBA| {
            cr.set_source_rgba(
                f64::from(c.red()),
                f64::from(c.green()),
                f64::from(c.blue()),
                f64::from(c.alpha()),
            );
        };
        let ink = view.color();
        let line = gdk::RGBA::new(ink.red(), ink.green(), ink.blue(), 0.3);
        let doc = self.doc.borrow();

        // GTK skips paragraph backgrounds on empty lines; fill them in so
        // code blocks stay unbroken.
        paint(crate::prose_tags::tint(view));
        let width = f64::from(view.width());
        for s in doc.spans.iter().filter(|s| {
            matches!(
                s.kind,
                xenon_prose::Kind::CodeBlock | xenon_prose::Kind::Verbatim
            )
        }) {
            let mut it = at(s.range.start);
            while (it.offset() as usize) < s.range.end {
                let (y, h) = view.line_yrange(&it);
                if y > bottom {
                    break;
                }
                if it.starts_line() && it.ends_line() && y + h >= top {
                    cr.rectangle(0.0, f64::from(y), width, f64::from(h));
                }
                if !it.forward_line() {
                    break;
                }
            }
        }
        let _ = cr.fill();

        paint(line);
        let x = f64::from(view.left_margin() + 6);
        for s in doc
            .spans
            .iter()
            .filter(|s| s.kind == xenon_prose::Kind::Quote)
        {
            let (y0, _) = view.line_yrange(&at(s.range.start));
            let (y1, h1) = view.line_yrange(&at(s.range.end));
            if y1 + h1 >= top && y0 <= bottom {
                cr.rectangle(x, f64::from(y0), 3.0, f64::from(y1 + h1 - y0));
            }
        }
        let _ = cr.fill();

        for row in &doc.rows {
            let (y, h) = view.line_yrange(&at(row.line.start));
            if y + h < top || y > bottom {
                continue;
            }
            let (a, b) = (
                view.iter_location(&at(row.line.start)),
                view.iter_location(&at(row.line.end)),
            );
            let (y, h) = (f64::from(y), f64::from(h));
            cr.rectangle(f64::from(a.x()), y + h - 1.0, f64::from(b.x() - a.x()), 1.0);
            if row.top {
                cr.rectangle(f64::from(a.x()), y, f64::from(b.x() - a.x()), 1.0);
            }
            for &p in &row.pipes {
                let r = view.iter_location(&at(p));
                cr.rectangle(f64::from(r.x() + r.width() / 2), y, 1.0, h);
            }
        }
        let _ = cr.fill();

        if !self.hide.get() {
            return;
        }
        paint(adw::StyleManager::default().accent_color_rgba());
        let shown = self.revealed.get();
        for m in doc.bullets_in(&(0..total)) {
            if shown.is_some_and(|_| m.block == shown) {
                continue;
            }
            let r = view.iter_location(&at(m.range.start));
            if r.y() + r.height() < top || r.y() > bottom {
                continue;
            }
            let (cx, cy) = (
                f64::from(r.x()) + f64::from(r.width()) / 2.0,
                f64::from(r.y()) + f64::from(r.height()) * 0.55,
            );
            cr.new_sub_path();
            cr.arc(cx, cy, (f64::from(r.height()) / 9.0).max(2.5), 0.0, TAU);
        }
        let _ = cr.fill();
    }
}
