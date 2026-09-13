use super::*;
use serde::Serialize;
use xenon_memory::ProcessMemory;

const MEMORY_REFRESH: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Serialize)]
pub(crate) struct MemoryTerminal {
    pub tab_id: u64,
    pub title: String,
    pub grid_bytes: u64,
    pub cols: u16,
    pub rows: u16,
    pub exited: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct MemorySnapshot {
    pub captured_at: String,
    pub process: ProcessMemory,
    pub history: Vec<MemorySample>,
    pub terminals: Vec<MemoryTerminal>,
    pub terminal_estimate_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct MemorySample {
    pub captured_at: String,
    pub process: ProcessMemory,
}

impl XenonApp {
    pub(crate) fn toggle_memory(&mut self, cx: &mut Context<Self>) {
        self.memory_panel = !self.memory_panel;
        if self.memory_task.is_none() {
            self.start_memory_monitor(cx);
        }
        cx.notify();
    }

    pub(crate) fn start_memory_monitor(&mut self, cx: &mut Context<Self>) {
        if self.memory_task.is_some() {
            return;
        }
        self.refresh_memory(cx);
        self.memory_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(MEMORY_REFRESH).await;
                if this.update(cx, |app, cx| app.refresh_memory(cx)).is_err() {
                    break;
                }
            }
        }));
    }

    fn refresh_memory(&mut self, cx: &mut Context<Self>) {
        let mut terminals = Vec::new();
        for content in self.contents.values() {
            if let Some(root) = &content.root {
                root.for_each_terminal(&mut |id, view| {
                    let term = view.read(cx);
                    let (cols, rows) = term.grid_size(cx).unwrap_or((0, 0));
                    terminals.push(MemoryTerminal {
                        tab_id: id.0,
                        title: term.title(cx),
                        grid_bytes: u64::from(cols) * u64::from(rows) * 24,
                        cols,
                        rows,
                        exited: term.is_exited(),
                    });
                });
            }
        }
        terminals.sort_by_key(|terminal| std::cmp::Reverse(terminal.grid_bytes));
        let terminal_estimate_bytes = terminals.iter().map(|terminal| terminal.grid_bytes).sum();
        let captured_at = chrono_like_now();
        let process = xenon_memory::process_memory();
        self.memory_history.push(MemorySample {
            captured_at: captured_at.clone(),
            process: process.clone(),
        });
        if self.memory_history.len() > 120 {
            self.memory_history.remove(0);
        }
        let snapshot = MemorySnapshot {
            captured_at,
            process,
            history: self.memory_history.clone(),
            terminals,
            terminal_estimate_bytes,
        };
        if let Err(error) =
            xenon_memory::write_snapshot(&xenon_store::data_dir().join("memory.json"), &snapshot)
        {
            log::debug!("memory snapshot write failed: {error}");
        }
        self.memory_snapshot = Some(snapshot);
    }

    pub(crate) fn render_memory_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let Some(snapshot) = &self.memory_snapshot else {
            return div().into_any_element();
        };
        let process = &snapshot.process;
        let mut rows = div().flex().flex_col().gap_1();
        for terminal in snapshot.terminals.iter().take(8) {
            rows = rows.child(
                div()
                    .flex()
                    .justify_between()
                    .gap_4()
                    .child(format!(
                        "{}  {}×{}",
                        compact_title(&terminal.title),
                        terminal.cols,
                        terminal.rows
                    ))
                    .child(format_bytes(terminal.grid_bytes)),
            );
        }
        div()
            .absolute()
            .top(px(48.))
            .right(px(16.))
            .w(px(390.))
            .max_h(px(560.))
            .p_4()
            .gap_3()
            .flex()
            .flex_col()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .text_color(colors.text)
            .shadow_lg()
            .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Memory"))
            .child(div().text_sm().text_color(colors.text_muted).child(format!("Updated {}", snapshot.captured_at)))
            .child(memory_metric("Footprint", process.footprint_bytes, colors.text_accent))
            .child(memory_metric("Peak", process.peak_footprint_bytes, colors.text_muted))
            .child(memory_metric("Resident", process.resident_bytes, colors.text_muted))
            .child(memory_metric("Compressed", process.compressed_bytes, colors.text_muted))
            .child(div().pt_2().text_sm().font_weight(gpui::FontWeight::SEMIBOLD).child("Tracked terminals"))
            .child(div().text_sm().text_color(colors.text_muted).child(format!("{} terminals, {} estimated grid memory", snapshot.terminals.len(), format_bytes(snapshot.terminal_estimate_bytes))))
            .child(rows)
            .child(div().pt_2().text_xs().text_color(colors.text_muted).child("Terminal estimates cover visible grids only. Process footprint includes native and GPU allocations. AI tools can read ~/.xenon/memory.json."))
            .into_any_element()
    }
}

fn memory_metric(label: &str, bytes: u64, color: gpui::Hsla) -> impl IntoElement {
    div()
        .flex()
        .justify_between()
        .child(div().text_color(color).child(label.to_string()))
        .child(format_bytes(bytes))
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{} B", bytes)
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn compact_title(title: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        "terminal".into()
    } else {
        title.chars().take(28).collect()
    }
}

fn chrono_like_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| format!("unix:{}", duration.as_secs()))
        .unwrap_or_else(|_| "unknown".into())
}
