//! Prose mode input and state: clicks, task boxes and links.

use super::*;

impl Prose {
    pub(crate) fn doc_images(&self) -> Vec<xenon_prose::ImageRef> {
        self.doc.borrow().images.clone()
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
