//! Git scenes: workspace-row markers, the Git section, and diff tabs.
//! Git state and diff text are fixed data; nothing here shells out.

use gpui::{Context, Window};
use xenon_core::WorkspaceRec;
use xenon_store::ThemeMode;

use super::{Scene, XenonApp, populate, set_theme};
use crate::app::WorkspaceDot;
use crate::diff_load::compose;
use crate::diff_model::LayoutMode;
use crate::git_dirt::{DirtState, GitFile, GitStatus};

fn theme(light: bool, cx: &mut Context<XenonApp>) {
    let mode = if light {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    };
    set_theme(mode, "One Dark", "One Light", cx);
}

fn file(index: char, worktree: char, path: &str, added: u32, removed: u32) -> GitFile {
    GitFile {
        index,
        worktree,
        path: path.to_string(),
        from: None,
        added,
        removed,
    }
}

fn dirty_files() -> Vec<GitFile> {
    let mut renamed = file('R', ' ', "docs/git-section.md", 3, 3);
    renamed.from = Some("docs/git-plan.md".to_string());
    vec![
        file('M', 'M', "crates/xenon_ui/src/sidebar/mod.rs", 3, 3),
        file(' ', 'M', "crates/xenon_ui/src/sidebar/widgets.rs", 13, 12),
        file(
            'A',
            ' ',
            "crates/xenon_ui/src/sidebar/git_section.rs",
            148,
            0,
        ),
        file('A', ' ', "crates/xenon_ui/src/app/git_section.rs", 96, 0),
        file(' ', 'M', "crates/xenon_ui/src/git_dirt.rs", 60, 130),
        renamed,
        file(' ', 'D', "docs/copy-clean-icon.md", 0, 41),
        file('?', '?', "tmp/git-section-notes.txt", 12, 0),
    ]
}

fn seed(app: &mut XenonApp, state: DirtState, files: Vec<GitFile>) {
    if let Some(id) = app.active {
        let branch = "main".to_string();
        app.services.git_dirt.insert(
            id,
            GitStatus {
                branch,
                state,
                files,
            },
        );
    }
}

/// Five workspace rows covering every marker combination.
pub(super) fn sidebar_markers(
    app: &mut XenonApp,
    light: bool,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    theme(light, cx);
    populate(app, window, cx);
    let home = std::env::var("HOME").unwrap_or_default();
    for name in ["homelab", "library", "aipm"] {
        let mut rec = WorkspaceRec::new(format!("{home}/src/{name}").into());
        rec.name = name.to_string();
        app.registry.workspaces.push(rec);
    }
    let others: Vec<_> = app
        .registry
        .workspaces
        .iter()
        .map(|w| w.id)
        .filter(|id| Some(*id) != app.active)
        .collect();
    let plan = [
        (
            WorkspaceDot::Attention("Idle after busy output"),
            DirtState::Dirty,
        ),
        (WorkspaceDot::Working, DirtState::IndexOnly),
        (WorkspaceDot::Attention("Bell"), DirtState::Clean),
        (WorkspaceDot::Working, DirtState::Clean),
    ];
    for (id, (status, state)) in others.iter().zip(plan) {
        app.services.visual_status.insert(*id, status);
        let git = GitStatus {
            branch: "main".into(),
            state,
            files: if state == DirtState::Clean {
                Vec::new()
            } else {
                vec![file('M', 'M', "README.md", 4, 4)]
            },
        };
        app.services.git_dirt.insert(*id, git);
    }
    if let [_, _, library, aipm] = others[..] {
        app.services.visual_status.remove(&library);
        app.services.visual_status.remove(&aipm);
        if let Some(git) = app.services.git_dirt.get_mut(&library) {
            git.state = DirtState::Dirty;
            git.files = vec![file(' ', 'M', "src/lib.rs", 2, 1)];
        }
    }
    if let Some(id) = app.active {
        app.services.visual_status.insert(id, WorkspaceDot::Working);
    }
    seed(app, DirtState::Dirty, dirty_files());
    cx.notify();
}

/// Git section expanded, collapsed, clean, or with the keyboard cursor.
pub(super) fn section(
    app: &mut XenonApp,
    scene: Scene,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    theme(scene == Scene::GitExpandedLight, cx);
    super::chrome::files_selected(app, window, cx);
    match scene {
        Scene::GitClean => seed(app, DirtState::Clean, Vec::new()),
        _ => seed(app, DirtState::Dirty, dirty_files()),
    }
    app.git.collapsed = scene == Scene::GitCollapsed;
    if scene == Scene::GitKeyboard {
        app.git.focused = true;
        app.git.cursor = 1;
        app.browser_focused = false;
    }
    cx.notify();
}

const WIDGETS_DIFF: &str =
    "diff --git a/crates/xenon_ui/src/sidebar/widgets.rs b/crates/xenon_ui/src/sidebar/widgets.rs
--- a/crates/xenon_ui/src/sidebar/widgets.rs
+++ b/crates/xenon_ui/src/sidebar/widgets.rs
@@ -59,6 +59,8 @@ pub(super) const ROW_H: f32 = 44.;
 pub(super) const ICON_SM: f32 = 14.;
 pub(super) const ICON_MD: f32 = 15.;
 /// Square hit target for pencil / close on workspace rows.
+/// Workspace dirty marker: one filled tick when the tree differs from HEAD.
+pub(super) const DIRTY_TICK: f32 = 18.;
 pub(super) const ACTION_BTN: f32 = 20.;
 /// Two action buttons (no gap) - keeps dirt gutter and hover overlay aligned.
 pub(super) const ACTIONS_W: f32 = ACTION_BTN * 2.;
@@ -86,30 +88,29 @@ pub(super) fn workspace_rename_row(
         .into_any_element()
 }
 
-/// Right-edge gutter for workspace rows: holds git +/- flush right.
+/// Right-edge gutter for workspace rows: one dirty tick, flush right.
 /// `min_w` matches hover-action width so absolute actions never cover the title.
 pub(super) fn workspace_dirt_gutter(
-    dirt: Option<crate::git_dirt::DirtLabels>,
+    dirt: DirtState,
     group: &str,
     colors: &theme::ThemeColors,
 ) -> impl IntoElement + use<> {
     let group = group.to_string();
     div()
         .flex_none()
         .w(px(ACTIONS_W))
         .flex()
         .items_center()
-        .justify_end()
-        .children(dirt.map(|labels| {
-            div()
-                .group_hover(group, |s| s.invisible())
-                .child(crate::git_dirt::badge(labels, colors.version_control_added))
-        }))
+        .justify_end()
+        .children(tick(dirt, &group, colors))
 }
";

const README_STAGED: &str = "@@ -172,9 +172,9 @@ Every Steam game tile runs vsynced to the 60 Hz virtual display
 - Measured (MangoHud, `steam.py bench`, 30 s, no stream attached), avg / 1% low fps / frametime stddev:
 
-| Game | Before | After vsync |
-| --- | --- | --- |
-| Black Mesa, Proton (Xen, 60 deg/s turn) | 92.5 / 81.2 / 0.60 ms / 0 | 60.0 / 55.1 / 0.64 ms / 0 |
+| Game (scene)                            | Before                    | After vsync               |
+| --------------------------------------- | ------------------------- | ------------------------- |
+| Black Mesa, Proton (Xen, 60 deg/s turn) | 92.5 / 81.2 / 0.60 ms / 0 | 60.0 / 55.1 / 0.64 ms / 0 |
 | Portal 2, native GL | 60.0 / 58.6 / 0.19 ms / 0 | 59.9 / 58.4 / 1.14 ms / 1 |
 
 Black Mesa went from uncapped 92 fps to a flat 60.
";

fn reverse(unified: &str) -> String {
    unified
        .lines()
        .map(|line| match line.as_bytes().first() {
            Some(b'-') => format!("+{}", &line[1..]),
            Some(b'+') => format!("-{}", &line[1..]),
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// Diff tab: split, unified, narrow Auto, or index-only (staged + unstaged).
pub(super) fn diff(
    app: &mut XenonApp,
    scene: Scene,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    theme(scene == Scene::DiffSplitLight, cx);
    populate(app, window, cx);
    let (path, doc, mode, beside) = match scene {
        Scene::DiffIndexOnly => {
            let path = "deploy/sunshine/README.md";
            let staged = README_STAGED.to_string();
            let unstaged = reverse(&staged);
            (
                path,
                compose(path, "", &staged, &unstaged),
                LayoutMode::Auto,
                false,
            )
        }
        _ => {
            let path = "crates/xenon_ui/src/sidebar/widgets.rs";
            let mode = if scene == Scene::DiffUnified {
                LayoutMode::Unified
            } else {
                LayoutMode::Auto
            };
            (
                path,
                compose(path, WIDGETS_DIFF, "", ""),
                mode,
                scene == Scene::DiffNarrowAuto,
            )
        }
    };
    let git_file = file(' ', 'M', path, doc.added, doc.removed);
    let mut files = dirty_files();
    if scene == Scene::DiffIndexOnly {
        files = vec![file('M', 'M', path, 4, 4)];
        seed(app, DirtState::IndexOnly, files);
    } else {
        files[1] = git_file.clone();
        seed(app, DirtState::Dirty, files);
    }
    app.open_scene_diff(git_file, mode, doc, beside, cx);
}
