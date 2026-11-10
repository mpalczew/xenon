use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::prose::Prose;

const MAX_HEIGHT: f64 = 420.0;
const GAP: i32 = 6;

struct Slot {
    picture: gtk::Picture,
    line_end: usize,
    height: i32,
}

type Cached = Option<(gdk::Texture, SystemTime)>;

#[derive(Default)]
pub(crate) struct Images {
    /// Pictures are kept and reused: overlay children cannot be taken out of
    /// a text view cleanly, so unused ones are only hidden.
    slots: RefCell<Vec<Slot>>,
    active: Cell<usize>,
    tags: RefCell<Vec<gtk::TextTag>>,
    textures: RefCell<HashMap<PathBuf, Cached>>,
    pending: Cell<bool>,
}

fn resolve(base: &Path, url: &str) -> Option<PathBuf> {
    if url.contains("://") && !url.starts_with("file://") {
        return None;
    }
    let url = url.strip_prefix("file://").unwrap_or(url);
    let url = glib::uri_unescape_string(url, None::<&str>)
        .map_or_else(|| url.to_string(), |s| s.to_string());
    let path = Path::new(&url);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.parent()?.join(path)
    })
}

impl Images {
    pub(crate) fn clear(&self, p: &Prose) {
        for slot in self.slots.borrow().iter() {
            slot.picture.set_visible(false);
        }
        self.active.set(0);
        let (a, b) = p.buffer.bounds();
        for tag in self.tags.borrow_mut().drain(..) {
            p.buffer.remove_tag(&tag, &a, &b);
        }
    }

    fn texture(&self, path: &Path) -> Option<gdk::Texture> {
        let stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
        if let Some(Some((t, at))) = self.textures.borrow().get(path)
            && *at == stamp
        {
            return Some(t.clone());
        }
        let loaded = gdk::Texture::from_filename(path).ok();
        self.textures
            .borrow_mut()
            .insert(path.to_path_buf(), loaded.clone().map(|t| (t, stamp)));
        loaded
    }

    pub(crate) fn rebuild(&self, p: &Rc<Prose>) {
        self.clear(p);
        let column = (p.scroller.width() - 2 * p.view.left_margin()).max(120);
        let images = p.doc_images();
        for image in images {
            let Some(path) = resolve(&p.path, &image.url) else {
                continue;
            };
            let Some(texture) = self.texture(&path) else {
                continue;
            };
            let (w, h) = (f64::from(texture.width()), f64::from(texture.height()));
            let fit = (f64::from(column) / w).min(MAX_HEIGHT / h).min(1.0);
            let (w, h) = ((w * fit) as i32, (h * fit) as i32);
            let name = format!("xp-img-{h}");
            let table = p.buffer.tag_table();
            let tag = table.lookup(&name).unwrap_or_else(|| {
                let tag = gtk::TextTag::new(Some(&name));
                tag.set_property("pixels-below-lines", h + GAP * 2);
                table.add(&tag);
                tag
            });
            let mut start = p.buffer.iter_at_offset(image.line_end as i32);
            start.set_line_offset(0);
            let end = p.buffer.iter_at_offset(image.line_end as i32);
            p.buffer.apply_tag(&tag, &start, &end);
            self.tags.borrow_mut().push(tag);
            let mut slots = self.slots.borrow_mut();
            let at = self.active.get();
            if let Some(slot) = slots.get_mut(at) {
                slot.picture.set_paintable(Some(&texture));
                slot.picture.set_size_request(w, h);
                slot.picture.set_visible(true);
                slot.line_end = image.line_end;
                slot.height = h;
            } else {
                let picture = gtk::Picture::for_paintable(&texture);
                picture.set_size_request(w, h);
                picture.set_can_shrink(false);
                picture.set_halign(gtk::Align::Start);
                p.view.add_overlay(&picture, 0, 0);
                slots.push(Slot {
                    picture,
                    line_end: image.line_end,
                    height: h,
                });
            }
            self.active.set(at + 1);
        }
        self.schedule_move(p);
    }

    pub(crate) fn schedule_move(&self, p: &Rc<Prose>) {
        if self.active.get() == 0 || self.pending.replace(true) {
            return;
        }
        let p = Rc::downgrade(p);
        glib::idle_add_local_once(move || {
            let Some(p) = p.upgrade() else { return };
            p.images.pending.set(false);
            p.images.place(&p);
        });
    }

    fn place(&self, p: &Prose) {
        let x = p.view.left_margin();
        for slot in self.slots.borrow().iter().take(self.active.get()) {
            let iter = p.buffer.iter_at_offset(slot.line_end as i32);
            let (y, height) = p.view.line_yrange(&iter);
            p.view
                .move_overlay(&slot.picture, x, y + height - slot.height - GAP);
        }
    }
}
