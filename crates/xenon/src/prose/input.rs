//! Prose mode input and state: clicks, task boxes and links.

use super::*;

impl Prose {
    pub(crate) fn doc_images(&self) -> Vec<xenon_prose::ImageRef> {
        self.doc.borrow().images.clone()
    }

    /// Enter: Ctrl flips the task box, a plain Enter continues a list.
    pub(crate) fn enter(self: &Rc<Self>, ctrl: bool, plain: bool) -> bool {
        self.on.get()
            && if ctrl {
                self.toggle_task()
            } else {
                plain && self.continue_list()
            }
    }

    /// Plain click on a task box flips it; Ctrl+click on a link opens it.
    pub(super) fn clicked(self: &Rc<Self>, x: f64, y: f64, ctrl: bool) {
        if !self.on.get() || self.buffer.has_selection() {
            return;
        }
        let (bx, by) =
            self.view
                .window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
        let Some(iter) = self.view.iter_at_location(bx, by) else {
            return;
        };
        let pos = iter.offset() as usize;
        let doc = self.doc.borrow();
        if ctrl {
            let url = doc.link_at(pos).map(|l| l.url.clone());
            drop(doc);
            if let Some(url) = url {
                self.open(&url);
            }
        } else if let Some(c) = doc.checkbox_at(pos).cloned() {
            drop(doc);
            self.flip(&c);
        }
    }

    /// Flip the task box on the caret's line (Ctrl+Enter).
    pub(crate) fn toggle_task(self: &Rc<Self>) -> bool {
        let found = self.doc.borrow().checkbox_on_line_of(self.caret()).cloned();
        match found {
            Some(c) => {
                self.flip(&c);
                true
            }
            None => false,
        }
    }

    /// Enter at the end of a list item: start the next item, or end the list
    /// when the item is empty. The edit is one undo step, made after the key.
    pub(crate) fn continue_list(self: &Rc<Self>) -> bool {
        let end = self.at(self.caret());
        if !self.on.get() || self.buffer.has_selection() || !end.ends_line() {
            return false;
        }
        let mut start = end;
        start.set_line_offset(0);
        let (a, b) = (start.offset(), end.offset());
        if !self.doc.borrow().is_item_line(a as usize) {
            return false;
        }
        let Some(action) = xenon_prose::list_enter(&self.buffer.text(&start, &end, true)) else {
            return false;
        };
        let this = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(this) = this.upgrade() else { return };
            let buffer = &this.buffer;
            if buffer.char_count() < b {
                return;
            }
            buffer.begin_user_action();
            match action {
                xenon_prose::Enter::End => {
                    buffer.delete(&mut buffer.iter_at_offset(a), &mut buffer.iter_at_offset(b));
                }
                xenon_prose::Enter::Continue(prefix) => {
                    buffer.place_cursor(&buffer.iter_at_offset(b));
                    buffer.insert_at_cursor(&format!("\n{prefix}"));
                }
            }
            buffer.end_user_action();
            this.view.scroll_mark_onscreen(&buffer.get_insert());
        });
        true
    }

    pub(super) fn flip(&self, c: &xenon_prose::Checkbox) {
        let at = c.range.start + 1;
        self.buffer.begin_user_action();
        self.buffer.delete(&mut self.at(at), &mut self.at(at + 1));
        self.buffer
            .insert(&mut self.at(at), if c.checked { " " } else { "x" });
        self.buffer.end_user_action();
    }

    pub(super) fn open(&self, url: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let url = glib::uri_unescape_string(url, None::<&str>)
            .map_or_else(|| url.to_string(), |s| s.to_string());
        if url.contains("://") || url.starts_with("mailto:") {
            app.launch(&url);
            return;
        }
        let target = url.split('#').next().unwrap_or_default();
        if target.is_empty() {
            return;
        }
        let dir = self.path.parent().unwrap_or(Path::new("."));
        let path = dir.join(target);
        if path.is_file() {
            app.open_editor(self.ws, &path);
        } else {
            app.toast(&format!("Not found: {}", path.display()));
        }
    }
}
