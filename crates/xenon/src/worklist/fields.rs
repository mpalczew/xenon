use gtk::prelude::*;

pub(super) fn title_view(placeholder: &str) -> (gtk::TextView, gtk::Widget) {
    let view = gtk::TextView::new();
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_accepts_tab(false);
    view.set_top_margin(6);
    view.set_bottom_margin(6);
    view.set_left_margin(8);
    view.set_right_margin(8);
    let hint = gtk::Label::new(Some(placeholder));
    hint.add_css_class("dim-label");
    hint.set_halign(gtk::Align::Start);
    hint.set_valign(gtk::Align::Start);
    hint.set_margin_top(6);
    hint.set_margin_start(8);
    hint.set_can_target(false);
    let shown = hint.clone();
    view.buffer().connect_changed(move |buffer| {
        let (shown, empty) = (shown.clone(), buffer.char_count() == 0);
        gtk::glib::idle_add_local_once(move || shown.set_visible(empty));
    });
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&view));
    overlay.add_overlay(&hint);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_child(Some(&overlay));
    scroller.set_has_frame(true);
    scroller.set_hexpand(true);
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_propagate_natural_height(true);
    scroller.set_max_content_height(120);
    (view, scroller.upcast())
}

pub(super) fn text_of(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    let (start, end) = buffer.bounds();
    buffer.text(&start, &end, false).to_string()
}
