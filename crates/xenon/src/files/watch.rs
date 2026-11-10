use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;

use super::{FilesView, draw_item, row_path, tree_row};

impl FilesView {
    pub(crate) fn reveal(self: &Rc<Self>, dir: &Path) {
        if !self
            .root_dir
            .borrow()
            .as_deref()
            .is_some_and(|root| dir.starts_with(root))
        {
            return;
        }
        *self.reveal_target.borrow_mut() = Some(dir.to_path_buf());
        self.advance_reveal();
    }

    pub(super) fn advance_reveal(&self) {
        let Some(target) = self.reveal_target.borrow().clone() else {
            return;
        };
        let Some(root) = self.root_dir.borrow().clone() else {
            return;
        };
        let mut chain: Vec<&Path> = target
            .ancestors()
            .take_while(|p| *p != root.as_path())
            .collect();
        chain.reverse();
        for path in chain {
            let Some((_, row)) = self.find(path) else {
                return;
            };
            if row.is_expandable() && !row.is_expanded() {
                row.set_expanded(true);
            }
        }
        self.reveal_target.take();
        self.select_path(&target, true);
    }

    pub(super) fn find(&self, path: &Path) -> Option<(u32, gtk::TreeListRow)> {
        (0..self.tree.n_items()).find_map(|i| {
            let row = tree_row(self.tree.item(i))?;
            (row_path(&row).as_deref() == Some(path)).then_some((i, row))
        })
    }

    pub(crate) fn mark_open(&self, path: Option<PathBuf>) {
        if *self.open_file.borrow() == path {
            return;
        }
        *self.open_file.borrow_mut() = path;
        for item in self.bound.borrow().iter() {
            draw_item(item, self.open_file.borrow().as_deref());
        }
    }
}
