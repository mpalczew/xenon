//! Ctrl+Shift+O: the workspace picker. Open and closed workspaces, folders
//! found by name under the usual project roots, and path completion for typed
//! paths. The rules are the GPUI picker's: known workspaces beat discovered
//! folders, a used workspace beats a random folder with a similar name, and a
//! missing folder is shown but never opened. Discovery is
//! `xenon_finder::discover`; it runs on a worker thread after a short debounce.

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk::{gio, glib};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};
use xenon_finder::discover::{
    DiscoverQuery, MatchQuality, discover, expand_user_path, list_parent_candidates,
    parse_discover_query, path_is_dir, ranking_needle, resolve_existing_dir, same_root,
};

use crate::app::App;
use crate::palette::{Handlers, PaletteRow};
use crate::sidebar::short_path;

const DEBOUNCE: Duration = Duration::from_millis(60);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Open,
    Closed,
    /// A folder on disk. `found`: discovered or completed, not typed exactly.
    Path {
        found: bool,
    },
}

#[derive(Clone)]
struct Candidate {
    kind: Kind,
    name: String,
    root: PathBuf,
    last_opened: u64,
    missing: bool,
}

impl Candidate {
    fn path(root: PathBuf, found: bool) -> Self {
        let name = root.file_name().map_or_else(
            || root.to_string_lossy().into_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        Self {
            kind: Kind::Path { found },
            name,
            root,
            last_opened: 0,
            missing: false,
        }
    }

    fn selectable(&self) -> bool {
        !self.missing && path_is_dir(&self.root)
    }

    /// Lower is better. Known workspaces always beat discovery.
    fn tier(&self) -> u8 {
        match self.kind {
            Kind::Path { found: false } => 0,
            Kind::Open | Kind::Closed if !self.missing => 1,
            Kind::Path { found: true } => 2,
            _ => 3,
        }
    }

    fn badge(&self) -> Option<&'static str> {
        match self.kind {
            Kind::Open => Some("open"),
            Kind::Closed if self.missing => Some("missing"),
            Kind::Closed => Some("closed"),
            Kind::Path { .. } => None,
        }
    }

    fn haystack(&self) -> String {
        format!("{} {}", self.name, self.root.display())
    }
}

/// Lower is better: exact basename match to the discovery needle first.
fn basename_rank(c: &Candidate, needle: Option<&str>) -> u8 {
    let Some(needle) = needle else { return 1 };
    match MatchQuality::of_basename(&c.name, needle) {
        Some(MatchQuality::Exact) => 0,
        Some(MatchQuality::Prefix) => 1,
        Some(MatchQuality::Contains) => 2,
        None => 3,
    }
}

fn compare(a: &Candidate, b: &Candidate, needle: Option<&str>) -> Ordering {
    a.tier()
        .cmp(&b.tier())
        .then_with(|| basename_rank(a, needle).cmp(&basename_rank(b, needle)))
        .then_with(|| b.last_opened.cmp(&a.last_opened))
        .then_with(|| {
            a.root
                .components()
                .count()
                .cmp(&b.root.components().count())
        })
        .then_with(|| a.root.as_os_str().len().cmp(&b.root.as_os_str().len()))
        .then_with(|| a.name.cmp(&b.name))
}

/// The known candidates that match `query`, best fuzzy score first.
fn filter_known(known: &[Candidate], query: &str) -> Vec<Candidate> {
    if query.is_empty() {
        return known.to_vec();
    }
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf = Vec::new();
    let mut scored: Vec<(u32, usize)> = known
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let text = c.haystack();
            pattern
                .score(Utf32Str::new(&text, &mut buf), &mut matcher)
                .map(|s| (s, i))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| known[i].clone()).collect()
}

/// Char indexes of `query` in `text`, for the letters to emphasise.
fn hits(text: &str, query: &str) -> Vec<u32> {
    if query.is_empty() {
        return Vec::new();
    }
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let (mut buf, mut out) = (Vec::new(), Vec::new());
    pattern.indices(Utf32Str::new(text, &mut buf), &mut matcher, &mut out);
    out.sort_unstable();
    out.dedup();
    out
}

/// Disk answers for one query: discovered folders and path completions.
struct Found {
    discovered: Vec<PathBuf>,
    completions: Vec<PathBuf>,
}

fn search_disk(query: &str) -> Found {
    let discovered = match parse_discover_query(query) {
        Some(q) if !matches!(q, DiscoverQuery::Exact(_)) => {
            discover(&q).into_iter().map(|f| f.path).collect()
        }
        _ => Vec::new(),
    };
    let completions = list_parent_candidates(query).unwrap_or_default();
    Found {
        discovered,
        completions,
    }
}

/// One open picker.
struct Session {
    known: Vec<Candidate>,
    found: RefCell<Found>,
    results: RefCell<Vec<Candidate>>,
    generation: Cell<u64>,
}

impl Session {
    /// Rebuild the ranked rows for `query` from known workspaces plus the
    /// latest disk answers.
    fn refilter(&self, query: &str) -> Vec<Candidate> {
        let mut results = filter_known(&self.known, query);
        let found = self.found.borrow();
        for path in found.discovered.iter().chain(found.completions.iter()) {
            if !results
                .iter()
                .any(|c| c.root == *path || same_root(&c.root, path))
            {
                results.push(Candidate::path(path.clone(), true));
            }
        }
        if let Some(typed) = expand_user_path(query).filter(|p| p.is_dir())
            && !results
                .iter()
                .any(|c| c.root == typed || same_root(&c.root, &typed))
        {
            results.insert(0, Candidate::path(typed, false));
        }
        let needle = ranking_needle(query);
        results.sort_by(|a, b| compare(a, b, needle.as_deref()));
        results
    }
}

impl App {
    fn known_candidates(&self) -> Vec<Candidate> {
        let model = self.model.borrow();
        let reg = &model.registry;
        let make = |kind: Kind, rec: &xenon_core::WorkspaceRec| Candidate {
            kind,
            name: rec.name.clone(),
            root: rec.root.clone(),
            last_opened: rec.last_opened.unwrap_or(0),
            missing: kind == Kind::Closed && !path_is_dir(&rec.root),
        };
        reg.workspaces
            .iter()
            .filter(|r| r.ssh.is_none())
            .map(|r| make(Kind::Open, r))
            .chain(
                reg.closed_workspaces
                    .iter()
                    .filter(|r| r.ssh.is_none())
                    .map(|r| make(Kind::Closed, r)),
            )
            .collect()
    }

    pub(crate) fn open_workspace_picker(self: &Rc<Self>) {
        let session = Rc::new(Session {
            known: self.known_candidates(),
            found: RefCell::new(Found {
                discovered: Vec::new(),
                completions: Vec::new(),
            }),
            results: RefCell::default(),
            generation: Cell::new(0),
        });
        let (wq, wa, wc) = (
            Rc::downgrade(self),
            Rc::downgrade(self),
            Rc::downgrade(self),
        );
        let (sq, sa, sc) = (session.clone(), session.clone(), session.clone());
        self.launcher.open(
            &self.win,
            Handlers {
                placeholder: "Folder or ssh://host/path\u{2026}",
                hint: "return opens  ·  tab completes  ·  esc closes",
                on_query: Box::new(move |query| {
                    if let Some(app) = wq.upgrade() {
                        app.picker_query(&sq, query);
                    }
                }),
                on_accept: Box::new(move |row| {
                    let Some(app) = wa.upgrade() else { return };
                    let chosen = sa.results.borrow().get(row).cloned();
                    if let Some(c) = chosen
                        && let Some(root) = resolve_existing_dir(&c.root)
                    {
                        app.launcher.close();
                        app.open_workspace(&root);
                    }
                }),
                on_complete: Some(Box::new(move |row| {
                    let Some(app) = wc.upgrade() else { return };
                    let chosen = sc.results.borrow().get(row).cloned();
                    if let Some(c) = chosen {
                        let mut text = short_path(&c.root.to_string_lossy());
                        if c.selectable() {
                            text.push('/');
                        }
                        app.launcher.set_text(&text);
                    }
                })),
            },
        );
    }

    fn picker_query(self: &Rc<Self>, session: &Rc<Session>, query: &str) {
        // Known workspaces answer at once; disk answers follow.
        session.found.replace(Found {
            discovered: Vec::new(),
            completions: Vec::new(),
        });
        self.picker_show(session, query);
        let token = session.generation.get().wrapping_add(1);
        session.generation.set(token);
        if query.trim().is_empty() {
            return;
        }
        let (weak, session, query) = (Rc::downgrade(self), session.clone(), query.to_string());
        let epoch = self.launcher.epoch();
        glib::spawn_future_local(async move {
            glib::timeout_future(DEBOUNCE).await;
            if session.generation.get() != token {
                return;
            }
            let q = query.clone();
            let Ok(found) = gio::spawn_blocking(move || search_disk(&q)).await else {
                return;
            };
            let Some(app) = weak.upgrade() else { return };
            if session.generation.get() != token
                || !app.launcher.is_open()
                || app.launcher.epoch() != epoch
            {
                return;
            }
            session.found.replace(found);
            app.picker_show(&session, &query);
        });
    }

    fn picker_show(&self, session: &Session, query: &str) {
        let results = session.refilter(query);
        let rows: Vec<PaletteRow> = results
            .iter()
            .map(|c| PaletteRow {
                title: c.name.clone(),
                hits: hits(&c.name, query),
                detail: Some(short_path(&c.root.to_string_lossy())),
                badge: c.badge().map(str::to_string),
                enabled: c.selectable(),
            })
            .collect();
        let empty = if query.is_empty() {
            "No workspaces \u{2014} type a path, name, or ~/src name"
        } else {
            "No match \u{2014} try ~/src name or Browse\u{2026}"
        };
        *session.results.borrow_mut() = results;
        self.launcher.set_rows(rows, empty);
    }
}
