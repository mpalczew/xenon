use super::*;

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
        }
        let colors = cx.theme().colors().clone();
        let find_bar = self.render_find_bar(&colors, cx);
        let base = div()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .relative()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::on_key))
            .on_action(cx.listener(|this, _: &Cut, _, cx| {
                this.cut_selection(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                this.copy_selection(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Paste, _, cx| {
                this.paste_clipboard(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Find, window, cx| {
                this.open_find(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindNext, _, cx| {
                this.find_next(cx);
            }))
            .on_action(cx.listener(|this, _: &FindPrevious, _, cx| {
                this.find_previous(cx);
            }))
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(colors.element_selected))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                if this.exited {
                    return;
                }
                this.focus.focus(window, cx);
                this.paste_paths(paths.paths(), cx);
                this.note_interaction(cx);
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_down))
            .size_full()
            .bg(colors.terminal_background);
        let base = if self.hovered_link.is_some() {
            base.cursor_pointer()
        } else {
            base
        };

        let chrome = self.hover_chrome(cx);
        let menu = self
            .context_menu
            .map(|position| self.render_context_menu(position, cx));
        let tooltip = self
            .hovered_link
            .as_ref()
            .map(|info| self.render_link_tooltip(info, cx));
        match &self.state {
            State::Ready(terminal) => base
                .children(find_bar)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .child(grid_canvas(
                            terminal.clone(),
                            cx.entity(),
                            self.focus.clone(),
                        ))
                        .child(chrome)
                        .children(tooltip),
                )
                .children(menu),
            State::Pending => base.children(find_bar).child(chrome).children(menu),
            State::Failed(error) => base
                .text_color(cx.theme().colors().text)
                .children(find_bar)
                .child(chrome)
                .child(error.clone()),
        }
    }
}

impl TerminalView {
    fn render_link_tooltip(
        &self,
        info: &HoverInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let anchor = Point {
            x: info.position.x + px(12.),
            y: info.position.y + px(20.),
        };
        deferred(
            anchored().position(anchor).child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.elevated_surface_background)
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(format!("⌘-click to open  {}", info.label)),
            ),
        )
        .with_priority(1)
    }

    fn render_context_menu(
        &self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let mut menu_box = div()
            .occlude()
            .flex()
            .flex_col()
            .min_w(px(180.))
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_md()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(|_, _, cx| cx.stop_propagation());
        if let Some(path) = self.menu_path.clone() {
            let editor_path = path.clone();
            menu_box = menu_box
                .child(
                    context_item("menu-open-editor", "Open in Editor", "", &colors).on_click(
                        cx.listener(move |this, _, _, cx| {
                            cx.emit(TerminalEvent::OpenInEditor(editor_path.clone()));
                            this.dismiss_menu(cx);
                        }),
                    ),
                )
                .child(
                    context_item("menu-open-default", "Open in Default App", "", &colors).on_click(
                        cx.listener(move |this, _, _, cx| {
                            cx.open_with_system(&path);
                            this.dismiss_menu(cx);
                        }),
                    ),
                );
        }
        let menu_box = menu_box
            .child(
                context_item("menu-cut", "Cut", "⌘X", &colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.cut_selection(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            )
            .child(
                context_item("menu-copy", "Copy", "⌘C", &colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.copy_selection(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            )
            .child(
                context_item("menu-paste", "Paste", "⌘V", &colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.paste_clipboard(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            );
        div()
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.dismiss_menu(cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| this.dismiss_menu(cx)),
            )
            .child(deferred(anchored().position(position).child(menu_box)).with_priority(1))
    }
}

fn context_item(
    id: &'static str,
    label: &'static str,
    shortcut: &'static str,
    colors: &theme::ThemeColors,
) -> gpui::Stateful<gpui::Div> {
    let hover = colors.element_selected;
    let muted = colors.text_muted;
    let text = colors.text;
    let row = div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .gap_6()
        .px_3()
        .py_1()
        .text_sm()
        .text_color(text)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(label);
    if shortcut.is_empty() {
        row
    } else {
        row.child(div().text_xs().text_color(muted).child(shortcut))
    }
}

fn grid_canvas(
    terminal: Entity<Terminal>,
    view: Entity<TerminalView>,
    focus: FocusHandle,
) -> impl IntoElement {
    canvas(
        move |bounds, window, cx| {
            let face = xenon_settings::terminal_font(cx);
            let font = grid::terminal_font(&face.family);
            layout(&terminal, &font, face.size, bounds, window, cx)
        },
        move |bounds, grid_layout, window, cx| {
            let size = px(xenon_settings::terminal_font(cx).size);
            let line_height = grid::line_height(size, LINE_HEIGHT_MULTIPLIER);
            grid::paint(&grid_layout, line_height, window, cx);
            window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
        },
    )
    .size_full()
}

fn layout(
    terminal: &Entity<Terminal>,
    font: &gpui::Font,
    font_size: f32,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> grid::GridLayout {
    let size = px(font_size);
    let line_height = grid::line_height(size, LINE_HEIGHT_MULTIPLIER);
    grid::layout(terminal, bounds, font, size, line_height, window, cx)
}
