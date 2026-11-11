use gtk::glib::translate::IntoGlib;
use gtk::prelude::*;
use gtk::{gdk, pango};
use xenon_prose::Kind;

const HEADING_SCALE: [f64; 4] = [1.7, 1.4, 1.2, 1.08];

pub(crate) struct Tags {
    heading: [gtk::TextTag; 4],
    quote: gtk::TextTag,
    code_block: gtk::TextTag,
    verbatim: gtk::TextTag,
    meta: gtk::TextTag,
    item: [gtk::TextTag; 4],
    done: gtk::TextTag,
    marker: gtk::TextTag,
    checkbox: gtk::TextTag,
    rule: gtk::TextTag,
    bold: gtk::TextTag,
    italic: gtk::TextTag,
    strike: gtk::TextTag,
    code: gtk::TextTag,
    link: gtk::TextTag,
    alt: gtk::TextTag,
    pub dim: gtk::TextTag,
    pub hide: gtk::TextTag,
    pub bullet: gtk::TextTag,
}

pub(crate) struct Look {
    pub margin: i32,
    pub gap: i32,
    pub hang: i32,
    pub mono: String,
}

fn tag(table: &gtk::TextTagTable, name: &str) -> gtk::TextTag {
    let tag = gtk::TextTag::new(Some(name));
    table.add(&tag);
    tag
}

impl Tags {
    pub(crate) fn new(table: &gtk::TextTagTable) -> Self {
        let t = |n: &str| tag(table, &format!("xp-{n}"));
        Self {
            heading: [t("h1"), t("h2"), t("h3"), t("h4")],
            quote: t("quote"),
            code_block: t("code-block"),
            verbatim: t("verbatim"),
            meta: t("meta"),
            item: [t("li0"), t("li1"), t("li2"), t("li3")],
            done: t("done"),
            marker: t("marker"),
            checkbox: t("checkbox"),
            rule: t("rule"),
            bold: t("bold"),
            italic: t("italic"),
            strike: t("strike"),
            code: t("code"),
            link: t("link"),
            alt: t("alt"),
            dim: t("dim"),
            hide: t("hide"),
            bullet: t("bullet"),
        }
    }

    pub(crate) fn all(&self) -> Vec<&gtk::TextTag> {
        let mut all: Vec<&gtk::TextTag> = self.heading.iter().collect();
        all.extend(self.item.iter());
        all.extend([
            &self.quote,
            &self.code_block,
            &self.verbatim,
            &self.meta,
            &self.done,
            &self.marker,
            &self.checkbox,
            &self.rule,
            &self.bold,
            &self.italic,
            &self.strike,
            &self.code,
            &self.link,
            &self.alt,
            &self.dim,
            &self.hide,
            &self.bullet,
        ]);
        all
    }

    pub(crate) fn for_kind(&self, kind: Kind) -> &gtk::TextTag {
        match kind {
            Kind::Heading(n) => &self.heading[usize::from(n.clamp(1, 4)) - 1],
            Kind::ListItem(d) => &self.item[usize::from(d.min(3))],
            Kind::Bold => &self.bold,
            Kind::Italic => &self.italic,
            Kind::Strike => &self.strike,
            Kind::Code => &self.code,
            Kind::CodeBlock => &self.code_block,
            Kind::Quote => &self.quote,
            Kind::ListMarker => &self.marker,
            Kind::Checkbox(_) => &self.checkbox,
            Kind::Done => &self.done,
            Kind::Link => &self.link,
            Kind::ImageAlt => &self.alt,
            Kind::Rule => &self.rule,
            Kind::Verbatim => &self.verbatim,
            Kind::Meta => &self.meta,
        }
    }

    pub(crate) fn style(&self, view: &impl IsA<gtk::Widget>, look: &Look) {
        let ink = view.color();
        let ink_at = |alpha: f32| gdk::RGBA::new(ink.red(), ink.green(), ink.blue(), alpha);
        let muted = ink_at(0.65);
        let accent = adw::StyleManager::default().accent_color_rgba();
        let (tint, chip) = (tint(view), ink_at(0.11));
        let rule = ink_at(0.3);
        let dim = muted;
        let bold = pango::Weight::Bold.into_glib();
        let mono = look.mono.as_str();
        for (i, tag) in self.heading.iter().enumerate() {
            let above = look.gap / 2 + [18, 14, 10, 6][i];
            set(
                tag,
                &[
                    ("weight", &bold),
                    ("scale", &HEADING_SCALE[i]),
                    ("pixels-above-lines", &above),
                    ("pixels-below-lines", &(look.gap / 2 + 4)),
                ],
            );
        }
        let quote_margin = look.margin + 20;
        set(
            &self.quote,
            &[
                ("style", &pango::Style::Italic),
                ("foreground-rgba", &muted),
                ("paragraph-background-rgba", &tint),
                ("left-margin", &quote_margin),
            ],
        );
        let code_margin = look.margin + 12;
        for tag in [&self.code_block, &self.verbatim] {
            set(
                tag,
                &[
                    ("family", &mono),
                    ("scale", &0.92f64),
                    ("paragraph-background-rgba", &tint),
                    ("left-margin", &code_margin),
                ],
            );
        }
        set(&self.verbatim, &[("wrap-mode", &gtk::WrapMode::None)]);
        set(
            &self.meta,
            &[
                ("family", &mono),
                ("scale", &0.9f64),
                ("foreground-rgba", &muted),
            ],
        );
        for (depth, tag) in self.item.iter().enumerate() {
            let at = look.margin + look.hang * 2 * depth as i32;
            set(tag, &[("left-margin", &at), ("indent", &(-look.hang))]);
        }
        set(&self.done, &[("foreground-rgba", &muted)]);
        set(
            &self.marker,
            &[("foreground-rgba", &accent), ("weight", &bold)],
        );
        set(
            &self.checkbox,
            &[
                ("family", &mono),
                ("foreground-rgba", &accent),
                ("weight", &bold),
            ],
        );
        set(
            &self.rule,
            &[
                ("scale", &0.15f64),
                ("foreground-rgba", &rule),
                ("paragraph-background-rgba", &rule),
                ("pixels-above-lines", &0),
                ("pixels-below-lines", &0),
            ],
        );
        self.style_inline(
            &Inks {
                ink,
                muted,
                accent,
                chip,
            },
            mono,
        );
        set(&self.dim, &[("foreground-rgba", &dim)]);
        set(&self.hide, &[("invisible", &true)]);
        set(
            &self.bullet,
            &[("foreground-rgba", &gdk::RGBA::new(0.0, 0.0, 0.0, 0.0))],
        );
    }

    fn style_inline(&self, c: &Inks, mono: &str) {
        let Inks {
            ink,
            muted,
            accent,
            chip,
        } = c;
        let bold = pango::Weight::Bold.into_glib();
        set(&self.bold, &[("weight", &bold)]);
        set(&self.italic, &[("style", &pango::Style::Italic)]);
        set(&self.strike, &[("strikethrough", &true)]);
        set(
            &self.code,
            &[
                ("family", &mono),
                ("scale", &0.92f64),
                ("background-rgba", chip),
                ("foreground-rgba", ink),
            ],
        );
        set(
            &self.link,
            &[
                ("foreground-rgba", accent),
                ("underline", &pango::Underline::Single),
            ],
        );
        set(
            &self.alt,
            &[("style", &pango::Style::Italic), ("foreground-rgba", muted)],
        );
    }
}

struct Inks {
    ink: gdk::RGBA,
    muted: gdk::RGBA,
    accent: gdk::RGBA,
    chip: gdk::RGBA,
}

fn set(tag: &gtk::TextTag, props: &[(&str, &dyn ToValue)]) {
    for (name, value) in props {
        tag.set_property_from_value(name, &value.to_value());
    }
}

/// The shading behind quotes and code blocks.
pub(crate) fn tint(view: &impl IsA<gtk::Widget>) -> gdk::RGBA {
    let ink = view.color();
    gdk::RGBA::new(ink.red(), ink.green(), ink.blue(), 0.07)
}
