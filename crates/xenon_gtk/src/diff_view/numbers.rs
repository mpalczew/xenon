//! The diff's line-number column: a GtkSourceView gutter renderer that prints
//! the number it was given for each buffer line (blank for filler lines).

use std::cell::RefCell;

use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene};
use sourceview::prelude::*;
use sourceview::subclass::prelude::*;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Numbers {
        pub numbers: RefCell<Vec<u32>>,
        pub color: RefCell<Option<gdk::RGBA>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Numbers {
        const NAME: &'static str = "XenonDiffNumbers";
        type Type = super::Numbers;
        type ParentType = sourceview::GutterRenderer;
    }

    impl ObjectImpl for Numbers {}
    impl WidgetImpl for Numbers {}

    impl GutterRendererImpl for Numbers {
        fn snapshot_line(
            &self,
            snapshot: &gtk::Snapshot,
            lines: &sourceview::GutterLines,
            line: u32,
        ) {
            let n = self
                .numbers
                .borrow()
                .get(line as usize)
                .copied()
                .unwrap_or(0);
            if n == 0 {
                return;
            }
            let obj = self.obj();
            let layout = obj.create_pango_layout(Some(&n.to_string()));
            let (w, _) = layout.pixel_size();
            let (y, _) = lines.line_yrange(line, sourceview::GutterRendererAlignmentMode::Cell);
            snapshot.save();
            snapshot.translate(&graphene::Point::new(
                (obj.width() - w - 6) as f32,
                y as f32,
            ));
            if let Some(color) = *self.color.borrow() {
                snapshot.append_layout(&layout, &color);
            }
            snapshot.restore();
        }
    }
}

glib::wrapper! {
    pub struct Numbers(ObjectSubclass<imp::Numbers>)
        @extends sourceview::GutterRenderer, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Numbers {
    /// A column of `numbers` (one per buffer line), `width` pixels wide.
    pub fn new(numbers: Vec<u32>, width: i32, color: gdk::RGBA) -> Self {
        let this: Self = glib::Object::new();
        *this.imp().numbers.borrow_mut() = numbers;
        *this.imp().color.borrow_mut() = Some(color);
        this.set_width_request(width);
        this
    }
}
