use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use adw::prelude::*;
use gtk::{gio, glib};
use xenon_finder::IgnoreMatcher;

use super::{draw_item, is_dir, path_of};
use crate::app::defer;

/// What the sidebar hides: ignore-file matches, `.git` and dotfiles. With
/// `show_hidden` on, they all show, dimmed. One filter serves every listing.
pub(super) struct Ignores {
    matcher: RefCell<Option<Arc<IgnoreMatcher>>>,
    pub(super) root: RefCell<Option<PathBuf>>,
    pub(super) show_hidden: Cell<bool>,
    pub(super) filter: gtk::CustomFilter,
    pub(super) bound: Rc<RefCell<Vec<gtk::ListItem>>>,
    pub(super) open: Rc<RefCell<Option<PathBuf>>>,
}

impl Ignores {
    pub(super) fn new(
        show_hidden: bool,
        bound: Rc<RefCell<Vec<gtk::ListItem>>>,
        open: Rc<RefCell<Option<PathBuf>>>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Ignores>| {
            let weak = weak.clone();
            Self {
                matcher: RefCell::new(None),
                root: RefCell::new(None),
                show_hidden: Cell::new(show_hidden),
                filter: gtk::CustomFilter::new(move |item| {
                    weak.upgrade()
                        .is_none_or(|this| this.show_hidden.get() || !this.hidden(item))
                }),
                bound,
                open,
            }
        })
    }

    pub(super) fn hidden(&self, item: &glib::Object) -> bool {
        let Some(path) = path_of(item) else {
            return false;
        };
        let dotfile = path
            .file_name()
            .is_some_and(|n| n.as_encoded_bytes().starts_with(b"."));
        dotfile
            || self
                .matcher
                .borrow()
                .as_ref()
                .is_some_and(|m| m.is_ignored(&path, is_dir(item)))
    }

    pub(super) fn redraw(&self) {
        for item in self.bound.borrow().iter() {
            draw_item(item, self.open.borrow().as_deref(), self);
        }
    }

    pub(super) fn set_show_hidden(&self, on: bool) {
        self.show_hidden.set(on);
        self.filter.changed(gtk::FilterChange::Different);
        self.redraw();
    }

    /// Re-read the ignore files off the main thread, then re-filter.
    pub(super) fn refresh(self: &Rc<Self>) {
        let Some(root) = self.root.borrow().clone() else {
            self.matcher.take();
            return;
        };
        let this = self.clone();
        glib::spawn_future_local(async move {
            let Ok(matcher) = gio::spawn_blocking(move || IgnoreMatcher::new(&root)).await else {
                return;
            };
            *this.matcher.borrow_mut() = Some(Arc::new(matcher));
            this.filter.changed(gtk::FilterChange::Different);
            this.redraw();
        });
    }

    /// An ignore file appeared or something was removed: refresh once, after the event.
    pub(super) fn watch(self: &Rc<Self>, list: &gtk::DirectoryList) {
        let this = Rc::downgrade(self);
        // sync: event-time read: only looks at file names; the refresh is deferred.
        list.connect_items_changed(move |list, pos, removed, added| {
            let touched = removed > 0
                || (pos..pos + added).any(|i| {
                    list.item(i)
                        .and_downcast::<gio::FileInfo>()
                        .is_some_and(|f| {
                            matches!(f.name().to_str(), Some(".gitignore" | ".ignore"))
                        })
                });
            if touched {
                defer(&this, |ig| ig.refresh());
            }
        });
    }
}
