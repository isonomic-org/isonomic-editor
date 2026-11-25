// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Renders the editor offscreen with the real renderer and saves a
//! PNG. Needs a GPU or a software Vulkan driver.
//!
//! `cargo run -p editor-gpui --example screenshot -- editor.png [caret|menu]`

extern crate ui as gpui;

use std::process::ExitCode;
use std::sync::Arc;

use editor_gpui::{EditorState, EditorView};
use editor_model::Selection;
use ui::assets::Assets;
use ui::component::ActiveTheme as _;
use ui::test::TestWindowExt as _;
use ui::{
    AppContext as _, Bounds, Context, Entity, Focusable as _, HeadlessAppContext, IntoElement,
    ParentElement as _, Render, Styled as _, Window, WindowBounds, WindowOptions, div, px, size,
};

const TOUR: &str = include_str!("../../../corpus/tour.md");

struct Host {
    editor: Entity<EditorView>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .p_6()
            .child(self.editor.clone())
    }
}

fn main() -> ExitCode {
    let out = std::env::args().nth(1).unwrap_or_else(|| "editor.png".to_owned());
    let mode = std::env::args().nth(2).unwrap_or_default();
    let (caret, menu) = (mode == "caret", mode == "menu");
    let mut cx = HeadlessAppContext::with_platform(
        ui::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        ui::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        ui::init(cx);
        editor_gpui::init(cx);
    });
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Default::default(),
            size: size(px(820.), px(720.)),
        })),
        focus: false,
        show: false,
        ..Default::default()
    };
    let mut made = None;
    let opened = cx.update(|cx| {
        ui::open_window(options, cx, |window, cx| {
            let state = cx.new(|cx| EditorState::from_markdown(TOUR, 1, cx));
            // Select part of the second block, so the frame shows a
            // selection as well as text; or place a caret in it.
            state.update(cx, |state, cx| {
                if let Some(id) = state.snapshot().flat().get(1).copied() {
                    let selection = if caret || menu {
                        Selection::caret(id, 28)
                    } else {
                        Selection::range(id, 14, 28)
                    };
                    state.select(selection, cx);
                }
            });
            made = Some(state.clone());
            let editor = cx.new(|cx| EditorView::new(state, cx));
            let focus = editor.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
            cx.new(|_| Host { editor })
        })
    });
    let Ok((handle, _)) = opened else {
        eprintln!("could not open an offscreen window");
        return ExitCode::FAILURE;
    };
    let _ = cx.update_window(handle, |_, window, cx| window.render_frame(cx));
    if menu {
        // The menu hangs from where the last frame drew the caret, so
        // it needs that frame first.
        let _ = cx.update_window(handle, |_, window, cx| {
            if let Some(state) = &made {
                state.update(cx, |state, cx| {
                    state.insert_text(" /", cx);
                    state.insert_text("h", cx);
                    state.step_menu(true, cx);
                });
            }
            window.render_frame(cx);
        });
    }
    match cx.capture_screenshot(handle) {
        Ok(image) => match image.save(&out) {
            Ok(()) => {
                println!("wrote {out}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("could not write {out}: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("offscreen rendering is not available: {error}");
            ExitCode::FAILURE
        }
    }
}
