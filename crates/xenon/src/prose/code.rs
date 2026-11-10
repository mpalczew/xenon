use std::ops::Range;

use super::*;

pub(super) type Lit = Rc<Vec<(Range<usize>, gtk::TextTag)>>;

const LIMIT: usize = 20_000;
const LOOK: [(&str, &str); 4] = [
    ("foreground-rgba", "foreground-set"),
    ("weight", "weight-set"),
    ("style", "style-set"),
    ("underline", "underline-set"),
];

impl Prose {
    pub(super) fn color_code(&self, doc: &Doc, a: usize, b: usize) {
        for block in doc
            .code
            .iter()
            .filter(|c| c.body.end > a && c.body.start < b)
        {
            let from = block.body.start;
            for (r, tag) in self.lit(&block.lang, &block.body).iter() {
                self.tag_range(tag, (r.start + from).max(a)..(r.end + from).min(b));
            }
        }
    }

    fn lit(&self, lang: &str, body: &Range<usize>) -> Lit {
        let text = self
            .buffer
            .text(&self.at(body.start), &self.at(body.end), true);
        let key = format!("{lang}\0{text}");
        if let Some(hit) = self.lit.borrow().get(&key) {
            return hit.clone();
        }
        let found = if body.len() > LIMIT {
            Vec::new()
        } else {
            self.highlight(lang, &text)
        };
        let mut cache = self.lit.borrow_mut();
        if cache.len() > 32 {
            cache.clear();
        }
        cache.entry(key).or_insert(Rc::new(found)).clone()
    }

    fn highlight(&self, lang: &str, text: &str) -> Vec<(Range<usize>, gtk::TextTag)> {
        let langs = sourceview::LanguageManager::default();
        let Some(lang) = langs
            .language(lang)
            .or_else(|| langs.guess_language(Some(&format!("x.{lang}")), None))
        else {
            return Vec::new();
        };
        let scratch = sourceview::Buffer::with_language(&lang);
        scratch.set_style_scheme(self.buffer.style_scheme().as_ref());
        scratch.set_text(text);
        let (mut it, end) = scratch.bounds();
        scratch.ensure_highlight(&it, &end);
        let mut out = Vec::new();
        loop {
            let from = it.offset() as usize;
            let tags = it.tags();
            let more = it.forward_to_tag_toggle(None::<&gtk::TextTag>);
            let to = it.offset() as usize;
            for tag in tags {
                if let (Some(mine), true) = (self.syntax_tag(&tag), from < to) {
                    out.push((from..to, mine));
                }
            }
            if !more {
                return out;
            }
        }
    }

    /// Scheme tags are anonymous, so ours is named by the look it carries.
    fn syntax_tag(&self, theirs: &gtk::TextTag) -> Option<gtk::TextTag> {
        let set: Vec<_> = LOOK
            .iter()
            .filter(|(_, flag)| theirs.property::<bool>(flag))
            .map(|(prop, _)| (*prop, theirs.property_value(prop)))
            .collect();
        if set.is_empty() {
            return None;
        }
        let name = set.iter().fold(String::from("xp-sv"), |name, (prop, v)| {
            match v.get::<gdk::RGBA>() {
                Ok(rgba) => format!("{name}-{prop}={rgba}"),
                Err(_) => format!("{name}-{prop}={v:?}"),
            }
        });
        let table = self.buffer.tag_table();
        if let Some(tag) = table.lookup(&name) {
            return Some(tag);
        }
        let ours = gtk::TextTag::new(Some(&name));
        for (prop, v) in &set {
            ours.set_property_from_value(prop, v);
        }
        table.add(&ours);
        self.syntax.borrow_mut().push(ours.clone());
        Some(ours)
    }

    pub(super) fn drop_syntax(&self) -> bool {
        let table = self.buffer.tag_table();
        self.lit.borrow_mut().clear();
        let old: Vec<_> = self.syntax.borrow_mut().drain(..).collect();
        for tag in &old {
            table.remove(tag);
        }
        !old.is_empty()
    }
}
