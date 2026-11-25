// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opens the editor in a window on a sample document.
//!
//! `cargo run -p editor-gpui --example playground`

extern crate ui as gpui;

use editor_gpui::{EditorState, EditorView};
use ui::assets::Assets;
use ui::component::ActiveTheme as _;
use ui::{
    AppContext as _, Bounds, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, WindowBounds,
    WindowOptions, div, px, size,
};

const TOUR: &str = include_str!("../../../corpus/tour.md");

struct Page {
    editor: Entity<EditorView>,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("page")
            .size_full()
            .overflow_y_scroll()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .p_8()
            .child(self.editor.clone())
    }
}

fn main() {
    ui::application().with_assets(Assets).run(|cx| {
        ui::init(cx);
        editor_gpui::init(cx);
        cx.activate(true);
        let bounds = Bounds::centered(None, size(px(900.), px(760.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        let opened = ui::open_window(options, cx, |window, cx| {
            let state = cx.new(|cx| EditorState::from_markdown(TOUR, 1, cx));
            let editor = cx.new(|cx| EditorView::new(state, cx));
            let focus = editor.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
            cx.new(|_| Page { editor })
        });
        if let Ok((window, _)) = opened {
            let _ = window.update(cx, |_, window, _| {
                window.activate_window();
                window.set_window_title("Isonomic Editor");
            });
        }
    });
}
