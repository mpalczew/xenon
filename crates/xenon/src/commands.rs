//! The one command table. Every window command is a block in `commands!`: its action name,
//! label, icon, keys, palette presence and handler. Callers write `Cmd::X`, never `"app.x"`.

use std::path::Path;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use vte::prelude::*;
use xenon_core::SplitAxis;
use xenon_store::AppSettings;

use crate::app::{App, defer};
use crate::model::Fold;

/// What a command's key does while the palette or a picker is open.
#[derive(Clone, Copy)]
pub(crate) enum InLauncher {
    Blocked,
    Runs,
    Step(i32),
}

enum Run {
    Plain(fn(&Rc<App>)),
    Toggle {
        initial: fn(&AppSettings) -> bool,
        set: fn(&Rc<App>, bool),
    },
    Path(fn(&Rc<App>, &Path)),
}

pub(crate) struct Spec {
    label: &'static str,
    short: Option<&'static str>,
    icon: &'static str,
    keys: &'static [&'static str],
    palette: bool,
    launcher: InLauncher,
    run: Run,
}

impl Spec {
    fn new(run: Run) -> Self {
        Self {
            label: "",
            short: None,
            icon: "",
            keys: &[],
            palette: true,
            launcher: InLauncher::Blocked,
            run,
        }
    }

    fn run(f: fn(&Rc<App>)) -> Self {
        Self::new(Run::Plain(f))
    }

    fn toggle(initial: fn(&AppSettings) -> bool, set: fn(&Rc<App>, bool)) -> Self {
        Self::new(Run::Toggle { initial, set })
    }

    fn path(f: fn(&Rc<App>, &Path)) -> Self {
        Self::new(Run::Path(f))
    }
}

macro_rules! commands {
    ($($variant:ident = $name:literal => $spec:expr,)*) => {
        #[derive(Clone, Copy)]
        pub(crate) enum Cmd { $($variant),* }

        impl Cmd {
            pub(crate) const ALL: &[Cmd] = &[$(Cmd::$variant),*];

            pub(crate) fn name(self) -> &'static str {
                match self { $(Cmd::$variant => $name),* }
            }

            pub(crate) fn action(self) -> &'static str {
                match self { $(Cmd::$variant => concat!("app.", $name)),* }
            }

            fn spec(self) -> Spec {
                match self { $(Cmd::$variant => $spec),* }
            }
        }
    };
}

commands! {
    NewTerminal = "new-terminal" => Spec {
        label: "New Terminal", icon: "utilities-terminal-symbolic", keys: &["<Control>n"],
        launcher: InLauncher::Step(1),
        ..Spec::run(|a| a.new_terminal())
    },
    NewWorkspace = "new-workspace" => Spec {
        label: "New Workspace\u{2026}", keys: &["<Control><Alt>n"],
        ..Spec::run(|a| a.new_workspace_dialog())
    },
    OpenWorkspace = "open-workspace" => Spec {
        label: "Open Workspace\u{2026}", short: Some("Open Workspace"),
        keys: &["<Control>o", "<Control><Shift>o"], launcher: InLauncher::Runs,
        ..Spec::run(|a| a.open_workspace_picker())
    },
    OpenFinder = "open-finder" => Spec {
        label: "Go to File\u{2026}", short: Some("Open File"), icon: "system-search-symbolic",
        keys: &["<Control>p"], launcher: InLauncher::Step(-1),
        ..Spec::run(|a| a.open_finder())
    },
    CommandPalette = "command-palette" => Spec {
        label: "Command Palette\u{2026}", keys: &["<Control><Shift>p"], palette: false,
        ..Spec::run(|a| a.open_command_palette())
    },
    RunTask = "run-task" => Spec {
        label: "Run Task\u{2026}", icon: "media-playback-start-symbolic",
        keys: &["<Control><Shift>r"],
        ..Spec::run(|a| a.open_task_picker())
    },
    RunBuildTask = "run-build-task" => Spec {
        label: "Run Build Task", keys: &["<Control><Shift>b"],
        ..Spec::run(|a| a.run_build_task())
    },
    CaptureWorklist = "capture-worklist" => Spec {
        label: "Add to Worklist", icon: "document-edit-symbolic", keys: &["<Control><Shift>k"],
        ..Spec::run(|a| a.capture_worklist())
    },
    OpenWorklist = "open-worklist" => Spec {
        label: "Open Worklist", short: Some("Worklist"), icon: "view-list-symbolic",
        keys: &["<Control><Alt>k"],
        ..Spec::run(|a| a.open_worklist())
    },
    UndoCapture = "undo-capture" => Spec {
        label: "Undo Worklist Capture", keys: &["<Control><Alt>z"],
        ..Spec::run(|a| a.undo_capture())
    },
    AllowFolderTasks = "allow-folder-tasks" => Spec {
        label: "Allow Folder Tasks", keys: &["<Control><Alt>Return"], palette: false,
        ..Spec::run(|a| a.answer_folder_tasks(true))
    },
    DenyFolderTasks = "deny-folder-tasks" => Spec {
        label: "Don\u{2019}t Allow Folder Tasks", keys: &["<Control><Alt>BackSpace"],
        palette: false,
        ..Spec::run(|a| a.answer_folder_tasks(false))
    },
    OpenInBrowser = "open-in-browser" => Spec {
        label: "Open in Browser", keys: &["<Control><Alt>o"],
        ..Spec::run(|a| a.open_in_browser())
    },
    CloseTab = "close-tab" => Spec {
        label: "Close Tab", keys: &["<Control>w"],
        ..Spec::run(|a| a.close_active_tab())
    },
    CloseOtherTabs = "close-other-tabs" => Spec {
        label: "Close Other Tabs", keys: &["<Control><Alt><Shift>w"],
        ..Spec::run(|a| a.close_other_tabs())
    },
    NextTab = "next-tab" => Spec {
        label: "Next Tab",
        keys: &["<Control>Tab", "<Control>braceright", "<Control><Shift>bracketright"],
        ..Spec::run(|a| a.step_tab(1))
    },
    PrevTab = "prev-tab" => Spec {
        label: "Previous Tab",
        keys: &["<Control><Shift>Tab", "<Control>braceleft", "<Control><Shift>bracketleft"],
        ..Spec::run(|a| a.step_tab(-1))
    },
    NextPane = "next-pane" => Spec {
        label: "Next Pane", keys: &["<Control>grave"],
        ..Spec::run(|a| a.next_pane())
    },
    NextWorkspace = "next-workspace" => Spec {
        label: "Next Workspace", icon: "go-down-symbolic", keys: &["<Control><Alt>Down"],
        ..Spec::run(|a| a.step_workspace(1))
    },
    PrevWorkspace = "prev-workspace" => Spec {
        label: "Previous Workspace", icon: "go-up-symbolic", keys: &["<Control><Alt>Up"],
        ..Spec::run(|a| a.step_workspace(-1))
    },
    MoveWorkspaceUp = "move-workspace-up" => Spec {
        label: "Move Workspace Up", keys: &["<Control><Alt><Shift>Page_Up"],
        ..Spec::run(|a| a.shift_workspace(-1))
    },
    MoveWorkspaceDown = "move-workspace-down" => Spec {
        label: "Move Workspace Down", keys: &["<Control><Alt><Shift>Page_Down"],
        ..Spec::run(|a| a.shift_workspace(1))
    },
    CloseWorkspace = "close-workspace" => Spec {
        label: "Close Workspace", keys: &["<Control><Alt>w"],
        ..Spec::run(|a| a.close_workspace())
    },
    SplitRight = "split-right" => Spec {
        label: "Split Right", icon: "xenon-split-right-symbolic", keys: &["<Control>backslash"],
        ..Spec::run(|a| a.split(SplitAxis::Horizontal))
    },
    SplitDown = "split-down" => Spec {
        label: "Split Down", icon: "xenon-split-down-symbolic",
        keys: &["<Control>bar", "<Control><Shift>backslash"],
        ..Spec::run(|a| a.split(SplitAxis::Vertical))
    },
    ParkPane = "park-pane" => Spec {
        label: "Reserve Pane to the Right", icon: "sidebar-show-right-symbolic",
        keys: &["<Control><Alt>backslash"],
        ..Spec::run(|a| a.park_pane())
    },
    FocusTerminal = "focus-terminal" => Spec {
        label: "Focus Terminal", keys: &["<Control>1"],
        ..Spec::run(|a| a.focus_terminal())
    },
    FocusSidebar = "focus-sidebar" => Spec {
        label: "Focus Sidebar", keys: &["<Control>3"],
        ..Spec::run(|a| a.focus_sidebar())
    },
    FocusGit = "focus-git" => Spec {
        label: "Focus Git", keys: &["<Control><Alt>g"],
        ..Spec::run(|a| a.focus_git())
    },
    ToggleSidebar = "toggle-sidebar" => Spec {
        label: "Toggle Sidebar", icon: "sidebar-show-symbolic", keys: &["<Control><Alt>b"],
        ..Spec::run(|a| a.toggle_sidebar())
    },
    ToggleFiles = "toggle-files" => Spec {
        label: "Toggle Files",
        ..Spec::run(|a| a.toggle_fold(Fold::Files))
    },
    ShowHiddenFiles = "show-hidden-files" => Spec {
        label: "Show Hidden Files",
        ..Spec::toggle(|s| s.show_hidden_files, |a, on| a.sidebar.files.set_hidden(on))
    },
    CycleDiffLayout = "cycle-diff-layout" => Spec {
        label: "Cycle Diff Layout", keys: &["<Control><Alt><Shift>d"],
        ..Spec::run(|a| a.cycle_diff_layout())
    },
    Find = "find" => Spec {
        label: "Find in Terminal", keys: &["<Control><Shift>f"],
        ..Spec::run(|a| {
            if let Some(view) = a.current_term() {
                view.find.open();
            }
        })
    },
    FindNext = "find-next" => Spec {
        label: "Find Next", keys: &["F3"],
        ..Spec::run(|a| {
            if let Some(view) = a.current_term() {
                view.term.search_find_next();
            }
        })
    },
    FindPrev = "find-prev" => Spec {
        label: "Find Previous", keys: &["<Shift>F3"],
        ..Spec::run(|a| {
            if let Some(view) = a.current_term() {
                view.term.search_find_previous();
            }
        })
    },
    Cut = "cut" => Spec {
        label: "Cut", icon: "edit-cut-symbolic",
        ..Spec::run(|a| a.cut())
    },
    Copy = "copy" => Spec {
        label: "Copy", icon: "edit-copy-symbolic", keys: &["<Control><Shift>c"],
        ..Spec::run(|a| {
            if let Some(view) = a.current_term() {
                view.term.copy_clipboard_format(vte::Format::Text);
            }
        })
    },
    CopyClean = "copy-clean" => Spec {
        label: "Copy Clean", icon: "xenon-copy-clean-symbolic", keys: &["<Control><Shift>y"],
        ..Spec::run(|a| a.copy_clean())
    },
    CopyCode = "copy-code" => Spec {
        label: "Copy Code",
        ..Spec::run(|a| a.copy_code())
    },
    Paste = "paste" => Spec {
        label: "Paste", icon: "edit-paste-symbolic",
        keys: &["<Control>v", "<Control><Shift>v"],
        ..Spec::run(|a| a.paste())
    },
    ZoomIn = "zoom-in" => Spec {
        label: "Zoom In", keys: &["<Control>equal", "<Control>plus"],
        ..Spec::run(|a| a.zoom(Some(1.0)))
    },
    ZoomOut = "zoom-out" => Spec {
        label: "Zoom Out", keys: &["<Control>minus"],
        ..Spec::run(|a| a.zoom(Some(-1.0)))
    },
    ZoomReset = "zoom-reset" => Spec {
        label: "Reset Zoom", keys: &["<Control>0"],
        ..Spec::run(|a| a.zoom(None))
    },
    Save = "save" => Spec {
        label: "Save", icon: "document-save-symbolic",
        ..Spec::run(|a| a.save_active_editor())
    },
    SaveAs = "save-as" => Spec {
        label: "Save As\u{2026}", keys: &["<Control><Shift>s"],
        ..Spec::run(|a| a.save_active_editor_as())
    },
    SaveAll = "save-now" => Spec {
        label: "Save All", launcher: InLauncher::Runs,
        ..Spec::run(|a| a.save_all())
    },
    Settings = "settings" => Spec {
        label: "Settings", icon: "emblem-system-symbolic", keys: &["<Control>comma"],
        launcher: InLauncher::Runs,
        ..Spec::run(|a| a.toggle_settings())
    },
    ConnectPhone = "connect-phone" => Spec {
        label: "Connect Phone", keys: &["<Control><Shift>m"], palette: false,
        ..Spec::run(|a| a.connect_phone())
    },
    ShowShortcuts = "show-shortcuts" => Spec {
        label: "Keyboard Shortcuts", keys: &["<Control>question", "<Control><Shift>slash"],
        ..Spec::run(show_shortcuts)
    },
    Quit = "quit" => Spec {
        label: "Close Window", keys: &["<Control>q"], launcher: InLauncher::Runs,
        ..Spec::run(|a| a.quit())
    },
    QuitEndTerminals = "quit-end-terminals" => Spec {
        label: "Quit and End Terminals\u{2026}", launcher: InLauncher::Runs,
        ..Spec::run(|a| a.quit_end_terminals())
    },
    OpenPathEditor = "open-path-editor" => Spec {
        label: "Open in Editor", palette: false,
        ..Spec::path(|a, path| {
            if let Some(ws) = a.model.borrow().active() {
                a.open_editor(ws, path);
            }
        })
    },
    OpenPathDefault = "open-path-default" => Spec {
        label: "Open in Default App", palette: false,
        ..Spec::path(|a, path| a.open_file(path))
    },
}

impl Cmd {
    pub(crate) fn label(self) -> &'static str {
        self.spec().label
    }

    /// The short label, else the label.
    pub(crate) fn short(self) -> &'static str {
        let spec = self.spec();
        spec.short.unwrap_or(spec.label)
    }

    pub(crate) fn icon(self) -> &'static str {
        self.spec().icon
    }

    /// The first accelerator GTK holds for the action, as a label. Empty before `install`.
    pub(crate) fn chord(self) -> Option<String> {
        let app = gio::Application::default().and_downcast::<gtk::Application>()?;
        let accels = app.accels_for_action(self.action());
        accels.first().map(|accel| label(accel))
    }

    /// "Label · Chord".
    pub(crate) fn tip(self) -> String {
        match self.chord() {
            Some(chord) => format!("{} \u{b7} {chord}", self.label()),
            None => self.label().to_string(),
        }
    }

    /// Sets the action name and the tooltip.
    pub(crate) fn bind(self, widget: &(impl IsA<gtk::Actionable> + IsA<gtk::Widget>)) {
        widget.set_action_name(Some(self.action()));
        widget.set_tooltip_text(Some(&self.tip()));
    }

    pub(crate) fn menu_item(self) -> gio::MenuItem {
        gio::MenuItem::new(Some(self.label()), Some(self.action()))
    }

    pub(crate) fn menu_item_for(self, path: &Path) -> gio::MenuItem {
        let item = gio::MenuItem::new(Some(self.label()), None);
        item.set_action_and_target_value(
            Some(self.action()),
            Some(&path.to_string_lossy().to_variant()),
        );
        item
    }

    pub(crate) fn palette() -> impl Iterator<Item = Cmd> {
        Cmd::ALL.iter().copied().filter(|cmd| cmd.spec().palette)
    }
}

/// An accelerator string as a label, such as "Ctrl+Return".
pub(crate) fn label(accel: &str) -> String {
    match gtk::accelerator_parse(accel) {
        Some((key, mods)) => gtk::accelerator_get_label(key, mods).to_string(),
        None => accel.to_string(),
    }
}

fn show_shortcuts(app: &Rc<App>) {
    let section = adw::ShortcutsSection::new(None);
    for cmd in Cmd::ALL.iter().filter(|cmd| !cmd.spec().keys.is_empty()) {
        section.add(adw::ShortcutsItem::from_action(cmd.label(), cmd.action()));
    }
    let dialog = adw::ShortcutsDialog::new();
    dialog.add(section);
    dialog.present(Some(&app.win));
}

/// Registers every accelerator. GTK keeps them by action name, so this runs before any
/// widget asks for a chord.
pub(crate) fn install_accels(app: &gtk::Application) {
    for cmd in Cmd::ALL {
        app.set_accels_for_action(cmd.action(), cmd.spec().keys);
    }
}

/// Registers every action, once.
pub(crate) fn install(app: &Rc<App>) {
    let settings = xenon_store::load_settings().unwrap_or_default();
    for cmd in Cmd::ALL {
        let spec = cmd.spec();
        let weak = Rc::downgrade(app);
        let action = match spec.run {
            Run::Plain(run) => {
                let action = gio::SimpleAction::new(cmd.name(), None);
                let launcher = spec.launcher;
                action.connect_activate(move |_, _| {
                    defer(&weak, move |app| {
                        if app.launcher.is_open() {
                            match launcher {
                                InLauncher::Blocked => return,
                                InLauncher::Step(n) => {
                                    app.launcher.step(n);
                                    return;
                                }
                                InLauncher::Runs => {}
                            }
                        }
                        run(&app);
                    });
                });
                action
            }
            Run::Toggle { initial, set } => {
                let action = gio::SimpleAction::new_stateful(
                    cmd.name(),
                    None,
                    &initial(&settings).to_variant(),
                );
                // sync: event-time read: only reads the requested state; the change is deferred.
                action.connect_change_state(move |_, value| {
                    if let Some(on) = value.and_then(|v| v.get::<bool>()) {
                        defer(&weak, move |app| set(&app, on));
                    }
                });
                action
            }
            Run::Path(run) => {
                let action = gio::SimpleAction::new(cmd.name(), Some(glib::VariantTy::STRING));
                action.connect_activate(move |_, param| {
                    if let Some(text) = param.and_then(|p| p.str()) {
                        let text = text.to_owned();
                        defer(&weak, move |app| run(&app, Path::new(&text)));
                    }
                });
                action
            }
        };
        app.gtk.add_action(&action);
    }
}
