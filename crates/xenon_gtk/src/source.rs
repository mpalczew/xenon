//! GtkSourceView tab with the Vim input method.

use std::rc::Rc;

use adw::prelude::*;
use sourceview::prelude::*;

use crate::Ctx;

pub(crate) fn open_source(ctx: &Rc<Ctx>, path: &str) {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let buffer = sourceview::Buffer::new(None);
    buffer.set_text(&text);
    if let Some(lang) =
        sourceview::LanguageManager::default().guess_language(Some(path), None::<&str>)
    {
        buffer.set_language(Some(&lang));
    }
    if let Some(scheme) = sourceview::StyleSchemeManager::default().scheme("Adwaita-dark") {
        buffer.set_style_scheme(Some(&scheme));
    }
    let view = sourceview::View::with_buffer(&buffer);
    view.set_monospace(true);
    view.set_show_line_numbers(true);
    view.set_hexpand(true);
    view.set_vexpand(true);

    // Vim input method: an IM context fed by a key controller in the capture phase.
    let vim = sourceview::VimIMContext::new();
    vim.set_client_widget(Some(&view));
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.set_im_context(Some(&vim));
    view.add_controller(keys);
    {
        let path = path.to_string();
        let buffer = buffer.clone();
        vim.connect_write(move |_, _, _| {
            let (a, b) = buffer.bounds();
            let text = buffer.text(&a, &b, false);
            let res = std::fs::write(&path, text.as_str());
            log!(
                "vim :w signal -> wrote {} bytes to {path}: {res:?}",
                text.len()
            );
        });
    }
    vim.connect_edit(|_, _, p| log!("vim :edit signal path={p:?}"));

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&view));
    let page = ctx.view.append(&scroll);
    page.set_title(path.rsplit('/').next().unwrap_or(path));
    ctx.view.set_selected_page(&page);
    view.grab_focus();
    // Kept alive for the selftest through widget data.
    unsafe { scroll.set_data("vim", vim) };
    unsafe { scroll.set_data("buffer", buffer) };
    unsafe { scroll.set_data("view", view) };
}
