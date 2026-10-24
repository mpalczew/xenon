//! Task terminals (Xenon-only): a PTY that runs one program and reports how it
//! exited, for VS Code task support.

use std::path::PathBuf;

use gpui::{App, Context};
use task::Shell;

use super::{State, TerminalEvent, TerminalView};

impl TerminalView {
    /// A task terminal: `program args` runs directly (no interactive shell) in
    /// `cwd`. The tab is titled `title`; exit arrives as `TaskFinished`.
    pub fn task(
        program: String,
        args: Vec<String>,
        cwd: PathBuf,
        env: Vec<(String, String)>,
        title: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, rx) = async_channel::bounded(1);
        let mut view = Self::with_shell(
            Some(cwd),
            env,
            Shell::WithArguments {
                program,
                args,
                title_override: Some(title),
            },
            Some(tx),
            cx,
        );
        view._task_wait = cx.spawn(async move |view, cx| {
            let status = rx.recv().await.ok().flatten();
            view.update(cx, |view, cx| view.task_finished(status, cx))
                .ok();
        });
        view
    }

    fn task_finished(&mut self, status: Option<std::process::ExitStatus>, cx: &mut Context<Self>) {
        let code = status.and_then(|s| s.code());
        self.task_exit = Some(code);
        self.working = false;
        self.wakeups = 0;
        cx.emit(TerminalEvent::TaskFinished(code));
        if !self.exited {
            self.exited = true;
            cx.emit(TerminalEvent::Exited);
        }
        cx.notify();
    }

    /// Exit code once a task terminal's process has finished.
    pub fn task_exit(&self) -> Option<Option<i32>> {
        self.task_exit
    }

    /// Ask for `TerminalEvent::Output` as output arrives.
    pub fn watch_output(&mut self) {
        self.watch_output = true;
    }

    /// Every line the terminal holds (scrollback and screen), oldest first.
    pub fn all_text(&self, cx: &App) -> Vec<String> {
        match &self.state {
            State::Ready(terminal) => terminal
                .read(cx)
                .get_content()
                .lines()
                .map(|l| l.trim_end().to_string())
                .collect(),
            _ => Vec::new(),
        }
    }
}
