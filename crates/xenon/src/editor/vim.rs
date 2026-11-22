use std::rc::{Rc, Weak};

use gtk::glib;
use gtk::prelude::*;
use sourceview::prelude::*;

use super::EditorView;
use crate::app::App;

impl EditorView {
    pub(super) fn attach_vim(self: &Rc<Self>, weak: &Weak<App>) {
        let vim = sourceview::VimIMContext::new();
        vim.set_client_widget(Some(&self.view));
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.set_im_context(Some(&vim));
        self.view.add_controller(keys.clone());
        let (this, weak) = (Rc::downgrade(self), weak.clone());
        vim.connect_write(move |_, _, _| {
            if let Some(this) = this.upgrade() {
                let weak = weak.clone();
                glib::idle_add_local_once(move || this.save(&weak));
            }
        });
        let status = self.vim_status(&vim);
        *self.vim.borrow_mut() = Some((vim, keys, status));
    }

    /// A line under the editor showing the Vim command bar and pending keys. While the
    /// bar holds a `/` or `?` search, matches are highlighted as it is typed; on Enter
    /// the context's own search takes over.
    fn vim_status(&self, vim: &sourceview::VimIMContext) -> gtk::Label {
        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        label.set_margin_start(8);
        label.add_css_class("monospace");
        label.set_visible(false);
        if let Some(root) = self.root.downcast_ref::<gtk::Box>() {
            root.append(&label);
        }
        let search =
            sourceview::SearchContext::new(&self.buffer, None::<&sourceview::SearchSettings>);
        search.set_highlight(false);
        search.settings().set_regex_enabled(true);
        search.settings().set_case_sensitive(true);
        let (label_in, search) = (label.clone(), search);
        let refresh = Rc::new(move |vim: &sourceview::VimIMContext| {
            let read = |name| vim.property::<Option<String>>(name).unwrap_or_default();
            let bar = read("command-bar-text");
            let text = if bar.is_empty() {
                read("command-text")
            } else {
                bar.clone()
            };
            label_in.set_text(&text);
            label_in.set_visible(!text.is_empty());
            let pattern = bar.strip_prefix(['/', '?']).filter(|p| !p.is_empty());
            search.settings().set_search_text(pattern);
            search.set_highlight(pattern.is_some());
        });
        let again = refresh.clone();
        vim.connect_command_bar_text_notify(move |vim| {
            let vim = vim.clone();
            let refresh = refresh.clone();
            glib::idle_add_local_once(move || refresh(&vim));
        });
        vim.connect_command_text_notify(move |vim| {
            let vim = vim.clone();
            let again = again.clone();
            glib::idle_add_local_once(move || again(&vim));
        });
        label
    }

    pub(super) fn detach_vim(&self) {
        let taken = self.vim.borrow_mut().take();
        if let Some((vim, keys, status)) = taken {
            if let Some(root) = self.root.downcast_ref::<gtk::Box>() {
                root.remove(&status);
            }
            self.view.remove_controller(&keys);
            vim.set_client_widget(None::<&gtk::Widget>);
        }
    }
}
