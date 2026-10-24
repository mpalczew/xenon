//! Task terminals as tabs: spawn the process, then place it per the task's
//! `presentation` (panel reuse, reveal, focus).

use super::*;
use xenon_core::{Panel, Reveal};

impl XenonApp {
    pub(super) fn spawn_task_terminal(
        &mut self,
        folder: &Path,
        task: &xenon_core::ResolvedTask,
        cx: &mut Context<Self>,
    ) -> Entity<TerminalView> {
        let mut env = self.terminal_env(folder);
        env.extend(task.env.iter().map(|(k, v)| (k.clone(), v.clone())));
        let (program, args) = if task.presentation.echo {
            // Echo the (secret-free) command line, then become the task.
            let mut args = vec![
                "-c".to_string(),
                "printf '\\033[2m> %s\\033[0m\\n\\n' \"$1\"; shift; exec \"$@\"".to_string(),
                "xenon-task".to_string(),
                task.display.clone(),
                task.program.clone(),
            ];
            args.extend(task.args.iter().cloned());
            ("/bin/sh".to_string(), args)
        } else {
            (task.program.clone(), task.args.clone())
        };
        let (cwd, label) = (task.cwd.clone(), task.label.clone());
        let view = cx.new(|cx| TerminalView::task(program, args, cwd, env, label, cx));
        view.update(cx, |view, cx| {
            // A task tab keeps its output; `presentation.close` closes it on success.
            view.set_auto_close(xenon_store::TerminalAutoClose::Off, cx);
            if task.is_background {
                view.watch_output();
            }
        });
        self.wire_terminal(&view, cx);
        view
    }

    /// Put the terminal in a tab per `presentation.panel` / `reveal` / `focus`.
    pub(super) fn place_task_terminal(
        &mut self,
        task: &xenon_core::ResolvedTask,
        view: Entity<TerminalView>,
        placement: super::task_runs::Placement,
        cx: &mut Context<Self>,
    ) {
        let super::task_runs::Placement {
            workspace,
            force_new,
            quiet,
        } = placement;
        let pres = task.presentation;
        let key = (workspace, task.label.clone());
        let reusable = if force_new {
            None
        } else {
            match pres.panel {
                Panel::New => None,
                Panel::Shared => self.services.tasks.shared_tabs.get(&workspace).cloned(),
                Panel::Dedicated => self.services.tasks.dedicated.get(&key).cloned(),
            }
            .filter(|old| old.read(cx).is_exited())
        };
        let select = !quiet && (force_new || pres.reveal == Reveal::Always);
        let replaced = reusable
            .and_then(|old| self.locate_terminal(&old))
            .is_some_and(|(ws, tab)| self.replace_terminal_tab(ws, tab, view.clone(), select));
        if !replaced {
            self.insert_terminal_tab(workspace, None, view.clone(), select);
        }
        match pres.panel {
            Panel::Shared => {
                self.services.tasks.shared_tabs.insert(workspace, view);
            }
            Panel::Dedicated => {
                self.services.tasks.dedicated.insert(key, view);
            }
            Panel::New => {}
        }
        if select && (pres.focus || force_new) {
            self.deferred.pending_focus = Some(FocusOwner::Terminal);
        } else if !quiet {
            self.deferred.pending_focus = self.deferred.restore_pane.take();
        }
        cx.notify();
    }

    /// Swap the terminal inside an existing tab (the tab id and place stay).
    fn replace_terminal_tab(
        &mut self,
        workspace: WorkspaceId,
        tab: TabId,
        view: Entity<TerminalView>,
        select: bool,
    ) -> bool {
        let Some(content) = self.contents.get_mut(&workspace) else {
            return false;
        };
        let Some((pane, index)) = content.root.as_ref().and_then(|r| r.find_tab(tab)) else {
            return false;
        };
        let Some(leaf) = content.root.as_mut().and_then(|r| r.find_leaf_mut(pane)) else {
            return false;
        };
        leaf.tabs[index] = LiveTab::Terminal { id: tab, view };
        if select {
            leaf.active = index;
        }
        true
    }

    /// Make a task's tab the visible one without moving focus.
    pub(super) fn reveal_terminal(&mut self, view: &Entity<TerminalView>) {
        let Some((workspace, tab)) = self.locate_terminal(view) else {
            return;
        };
        if let Some(content) = self.contents.get_mut(&workspace)
            && let Some((pane, index)) = content.root.as_ref().and_then(|r| r.find_tab(tab))
            && let Some(leaf) = content.root.as_mut().and_then(|r| r.find_leaf_mut(pane))
        {
            leaf.active = index;
        }
    }
}
