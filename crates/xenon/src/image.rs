use std::path::Path;

use adw::prelude::*;
use gtk::{gio, glib};

pub(crate) fn is_image(path: &Path) -> bool {
    let (kind, _) = gio::content_type_guess(Some(path), None::<&[u8]>);
    kind.starts_with("image/")
}

pub(crate) fn view(path: &Path) -> (gtk::Widget, Option<gio::FileMonitor>) {
    let picture = gtk::Picture::for_filename(path);
    picture.set_content_fit(gtk::ContentFit::ScaleDown);
    picture.set_can_shrink(true);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_child(Some(&picture));
    scroller.set_focusable(true);
    scroller.set_hexpand(true);
    scroller.set_vexpand(true);
    let monitor = gio::File::for_path(path)
        .monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
        .ok();
    if let Some(monitor) = &monitor {
        let (weak, path) = (picture.downgrade(), path.to_path_buf());
        monitor.connect_changed(move |_, _, _, event| {
            if event != gio::FileMonitorEvent::ChangesDoneHint {
                return;
            }
            let (weak, path) = (weak.clone(), path.clone());
            glib::idle_add_local_once(move || {
                if let Some(picture) = weak.upgrade() {
                    picture.set_filename(Some(&path));
                }
            });
        });
    }
    (scroller.upcast(), monitor)
}
