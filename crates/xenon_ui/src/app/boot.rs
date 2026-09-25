use super::*;

pub(super) enum BootKind {
    Normal,
    #[cfg(feature = "visual-tests")]
    Visual,
}

impl XenonApp {
    pub(super) fn boot(cx: &mut Context<Self>, kind: BootKind) -> Self {
        let registry = xenon_store::load_registry().unwrap_or_default();
        let settings = xenon_store::load_settings().unwrap_or_default();
        let (lsp, lsp_events) = LspState::new(settings.lsp.clone());
        xenon_settings::apply(&settings, cx);
        xenon_terminal::apply_theme(cx);
        let mut app = Self::from_loaded(registry, settings, lsp, cx);
        app.load_sessions();
        match kind {
            BootKind::Normal => {
                app.start_ide_server(cx);
                app.start_memory_monitor(cx);
                app.start_lsp_events(lsp_events, cx);
                app.restart_git_dirt_watch(cx);
                if let Err(error) = xenon_store::refresh_on_launch() {
                    log::warn!("skill refresh: {error}");
                }
            }
            #[cfg(feature = "visual-tests")]
            BootKind::Visual => {
                app.skip_persist = true;
                xenon_design_system::freeze_motion(cx);
                drop(lsp_events);
            }
        }
        Self::register_main_handle(cx);
        let active = app
            .registry
            .active
            .map(|a| a.workspace)
            .filter(|id| app.registry.workspace(*id).is_some())
            .or_else(|| app.first_workspace());
        if let Some(id) = active {
            app.activate_workspace(id, cx);
        }
        app
    }

    #[cfg(feature = "visual-tests")]
    pub fn new_visual(cx: &mut Context<Self>) -> Self {
        Self::boot(cx, BootKind::Visual)
    }

    fn from_loaded(
        registry: Registry,
        settings: xenon_store::AppSettings,
        lsp: LspState,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            registry,
            sessions: HashMap::new(),
            contents: HashMap::new(),
            active: None,
            finder: None,
            task_picker: None,
            worklist_captures: HashMap::new(),
            worklist_capture_subs: Vec::new(),
            worklist_capture_visible: None,
            worklist_undo: None,
            worklist_notice: xenon_design_system::TimedNotice::default(),
            workspace_picker: None,
            workspace_create: None,
            command_palette: None,
            theme_picker: None,
            browser_focused: false,
            file_indexes: HashMap::new(),
            index_tasks: HashMap::new(),
            recent_files: HashMap::new(),
            nav_history: HashMap::new(),
            nav_suppress: false,
            deferred: DeferredUi::default(),
            sidebar_collapsed: false,
            sidebar_width: xenon_core::clamp_sidebar(settings.sidebar_width),
            layout_dirty: false,
            workspaces_collapsed: settings.workspaces_collapsed,
            workspace_section_closing: false,
            workspace_section_animation: None,
            workspaces_section_height: settings.workspaces_section_height,
            file_browser: FileBrowser::with_open(settings.files_open),
            files_section_closing: false,
            files_section_animation: None,
            settings_window: None,
            renaming: None,
            _rename_sub: None,
            tab_menu: None,
            overflow_menu: None,
            tab_strip_widths: HashMap::new(),
            dragging_tab: None,
            browser_menu: None,
            workspace_menu: None,
            focus: cx.focus_handle(),
            _finder_sub: None,
            _task_picker_sub: None,
            _workspace_picker_sub: None,
            _workspace_create_sub: None,
            _command_palette_sub: None,
            _theme_picker_sub: None,
            attention: AttentionMap::default(),
            _bell_subs: Vec::new(),
            _selection_subs: Vec::new(),
            lsp,
            services: AppServices::default(),
            memory_panel: false,
            memory_snapshot: None,
            memory_history: Vec::new(),
            memory_task: None,
            skip_persist: false,
            skill_prompt: None,
            skill_skipped_session: false,
        }
    }
}
