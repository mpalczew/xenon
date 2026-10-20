//! Sidebar Git section: branch and file count in the header, one row per
//! changed file (status badges, colored name, dimmed directory, change bar),
//! or a two-line Clean body.

use gpui::{
    AnyElement, Context, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;
use xenon_design_system::{TypeRole, Typography};

use super::marks::kind_color;
use super::widgets::{id_hash, path_tooltip};
use crate::app::XenonApp;
use crate::git_dirt::{BarCell, GitFile, GitStatus, change_bar};
use crate::icons::icon;
use crate::resize::{GIT_HEADER_H, GIT_ROW_H};

impl XenonApp {
    /// The Git section under Files; `None` without a git status (SSH,
    /// non-git folder, or before the first measure).
    pub(crate) fn render_git_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let git = self.active_git()?.clone();
        let colors = cx.theme().colors().clone();
        let collapsed = self.git.collapsed && !self.git.closing;
        let body_h = self
            .git
            .height
            .unwrap_or_else(|| crate::resize::git_body_content_height(git.files.len()));
        let body = (!collapsed).then(|| {
            let rows = self.git_body(&git, cx);
            xenon_design_system::section_reveal(
                rows,
                ("git-content", self.git.closing as u32),
                cx,
                !self.git.closing,
            )
        });
        Some(
            div()
                .id("git-section")
                .relative()
                .flex_none()
                .flex()
                .flex_col()
                .w_full()
                .h(px(if collapsed {
                    GIT_HEADER_H
                } else {
                    GIT_HEADER_H + body_h
                }))
                .when(!collapsed, |s| {
                    s.child(crate::resize::row_resize_handle_top(
                        "sidebar-git-resize",
                        crate::resize::ResizeEdge::SidebarGit,
                        colors.border,
                    ))
                })
                .child(self.git_header(&git, cx))
                .children(body)
                .into_any_element(),
        )
    }

    fn git_header(&self, git: &GitStatus, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let border = if self.git.focused {
            colors.border_focused
        } else {
            colors.border
        };
        let count = git.files.len();
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(GIT_HEADER_H))
            .px_4()
            .border_t_1()
            .border_color(border)
            .child(
                xenon_design_system::action_button(
                    "git-section-header",
                    xenon_design_system::ActionButton::quiet("GIT")
                        .hover_text(colors.text)
                        .hover_background(gpui::transparent_black()),
                    cx,
                    cx.listener(|this, _, _, cx| this.toggle_git_section(cx)),
                )
                .justify_start()
                .px_0()
                .text_color(colors.text_muted)
                .tooltip(path_tooltip(SharedString::from(
                    xenon_design_system::shortcut_text("Git · ⌘⌥G"),
                ))),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .min_w_0()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text_muted)
                    .child(icon(Icon::GitBranch, px(13.)))
                    .child(div().truncate().max_w(px(110.)).child(git.branch.clone())),
            )
            .children((count > 0).then(|| {
                div()
                    .ml_2()
                    .px(px(7.))
                    .min_w(px(20.))
                    .flex()
                    .justify_center()
                    .rounded_full()
                    .bg(colors.element_selected)
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text)
                    .child(count.to_string())
            }))
    }

    fn git_body(&self, git: &GitStatus, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let colors = cx.theme().colors().clone();
        let body = div()
            .id("git-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.git.scroll);
        if git.files.is_empty() {
            let ok = colors.version_control_added;
            return body
                .px_4()
                .pt_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .type_role(TypeRole::ListPrimary, cx)
                        .child(div().text_color(ok).child(icon(Icon::CheckCircle, px(15.))))
                        .child("Clean"),
                )
                .child(
                    div()
                        .type_role(TypeRole::ControlLabel, cx)
                        .text_color(colors.text_muted)
                        .child(format!("Matches HEAD · {}", git.branch)),
                );
        }
        let open = self
            .active_content()
            .and_then(|c| c.active_tab())
            .and_then(|tab| tab.diff_path())
            .map(str::to_string);
        let cursor = self.git_cursor();
        body.children(git.files.iter().enumerate().map(|(i, file)| {
            let marked = self.git.focused && i == cursor;
            let shown = open.as_deref() == Some(file.path.as_str());
            self.git_row(file, marked || shown, marked, cx)
        }))
    }

    fn git_row(
        &self,
        file: &GitFile,
        selected: bool,
        marked: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let tint = kind_color(file.kind(), &colors, cx);
        let path = file.path.clone();
        let tip = match &file.from {
            Some(from) => format!("{from} → {}  +{} -{}", file.path, file.added, file.removed),
            None => format!("{}  +{} -{}", file.path, file.added, file.removed),
        };
        let dir = match &file.from {
            Some(from) => format!("← {from}"),
            None => file.dir_tail(),
        };
        xenon_design_system::selectable_row(
            ("git-row", id_hash(file.path.clone())),
            selected,
            &colors,
        )
        .h(px(GIT_ROW_H))
        .mx_2()
        .px_2()
        .py_0()
        .gap_2()
        .type_role(TypeRole::Body, cx)
        .tooltip(path_tooltip(SharedString::from(tip)))
        .child(div().flex().flex_none().gap(px(2.)).children(
            file.badges().into_iter().map(|(letter, kind)| {
                badge(letter, kind_color(kind, &colors, cx), colors.text_muted)
            }),
        ))
        .child(
            div()
                .flex_none()
                .max_w(px(150.))
                .truncate()
                .text_color(tint)
                .font_weight(if marked {
                    gpui::FontWeight::MEDIUM
                } else {
                    gpui::FontWeight::NORMAL
                })
                .when(file.kind() == crate::git_dirt::ChangeKind::Deleted, |d| {
                    d.line_through()
                })
                .child(file.name().to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(colors.text_muted)
                .child(dir),
        )
        .child(bars(file, &colors))
        .on_click(cx.listener(move |this, _, _, cx| this.open_git_diff(&path, cx)))
        .into_any_element()
    }
}

/// Round tinted status letter.
fn badge(letter: char, color: Hsla, muted: Hsla) -> impl IntoElement {
    let color = if letter == '?' { muted } else { color };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(18.))
        .rounded_full()
        .bg(color.opacity(0.18))
        .text_color(color)
        .text_size(px(10.))
        .font_weight(gpui::FontWeight::BOLD)
        .child(letter.to_string())
}

/// Five-cell green/red change bar.
fn bars(file: &GitFile, colors: &theme::ThemeColors) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .gap(px(2.))
        .children(change_bar(file.added, file.removed).map(|cell| {
            let color = match cell {
                BarCell::Added => colors.version_control_added,
                BarCell::Removed => colors.version_control_deleted,
                BarCell::Neutral => colors.text_muted.opacity(0.3),
            };
            div().w(px(4.)).h(px(10.)).rounded(px(1.)).bg(color)
        }))
}
