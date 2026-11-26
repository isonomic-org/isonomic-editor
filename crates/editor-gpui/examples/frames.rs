// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Times the editor's own share of a frame: taking a snapshot, building
//! the rows in view, laying them out and painting them into a scene.
//! The time the renderer then takes to draw that scene is not included.
//!
//! `cargo run --release -p editor-gpui --example frames [blocks]`

extern crate ui as gpui;

use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use editor_gpui::{EditorState, EditorView};
use editor_model::Selection;
use ui::assets::Assets;
use ui::{AppContext as _, Bounds, HeadlessAppContext, WindowBounds, WindowOptions, px, size};

const FRAMES: usize = 200;

fn document(blocks: usize) -> String {
    let mut out = String::new();
    for index in 0..blocks {
        match index % 10 {
            0 => out.push_str(&format!("## Section {}\n\n", index / 10 + 1)),
            3 | 4 => out.push_str(&format!("- item {index} with **bold** and `code`\n\n")),
            _ => out.push_str(&format!(
                "Paragraph {index}. A document is a tree of blocks, and this one is long \
                 enough to wrap in a narrow window so that rows differ in height.\n\n"
            )),
        }
    }
    out
}

fn report(label: &str, times: &mut [Duration], drawn: usize) {
    times.sort();
    let mean = times.iter().sum::<Duration>() / times.len() as u32;
    let worst = times[times.len() * 99 / 100];
    println!(
        "{label:<34} {drawn:>4} rows drawn   mean {:>6.2} ms   99th {:>6.2} ms",
        mean.as_secs_f64() * 1e3,
        worst.as_secs_f64() * 1e3
    );
}

fn main() -> ExitCode {
    let blocks = std::env::args().nth(1).and_then(|n| n.parse().ok()).unwrap_or(10_000);
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
            size: size(px(900.), px(1200.)),
        })),
        focus: false,
        show: false,
        ..Default::default()
    };
    // Save a document read from Markdown, then open the saved bytes
    // the way a host does: read off the drawing thread, then make the
    // state and draw the first frame on it.
    let text = document(blocks);
    let started = Instant::now();
    let saved = cx.update(|cx| {
        let state = cx.new(|cx| EditorState::from_markdown(&text, 1, cx));
        state.read(cx).save()
    });
    let Ok(saved) = saved else { return ExitCode::FAILURE };
    let imported = started.elapsed();

    let started = Instant::now();
    let Ok(read) = std::thread::spawn(move || EditorState::open(&saved, 2)).join() else {
        return ExitCode::FAILURE;
    };
    let Ok(read) = read else { return ExitCode::FAILURE };
    let reading = started.elapsed();
    let mut made = None;
    let opened = cx.update(|cx| {
        ui::open_window(options, cx, |_, cx| {
            let state = cx.new(|_| read);
            made = Some(state.clone());
            cx.new(|cx| EditorView::new(state, cx))
        })
    });
    let (Ok((handle, editor)), Some(state)) = (opened, made) else {
        eprintln!("could not open an offscreen window");
        return ExitCode::FAILURE;
    };
    let drawing = Instant::now();
    let first = cx.update_window(handle, |_, window, cx| window.draw(cx).clear(cx));
    if first.is_err() {
        return ExitCode::FAILURE;
    }
    let drawing = drawing.elapsed();
    let count = cx.update(|cx| state.read(cx).snapshot().len());
    let ms = |time: Duration| time.as_secs_f64() * 1e3;
    println!("{count} blocks");
    println!("read from Markdown and saved         {:>6.0} ms", ms(imported));
    // Making the window is the host's cost and the same for any
    // document, so it is left out.
    println!(
        "opened from saved bytes              {:>6.0} ms to the first frame: {:.0} ms reading, off \
         the drawing thread, and {:.1} ms drawing",
        ms(reading + drawing),
        ms(reading),
        ms(drawing)
    );

    // Type in the middle of the document, one character a frame.
    let middle = cx.update(|cx| state.read(cx).snapshot().flat()[count / 2]);
    cx.update(|cx| state.update(cx, |state, cx| state.select(Selection::caret(middle, 0), cx)));
    let mut passes: [(&str, Vec<Duration>); 2] = [
        ("typing a character a frame", Vec::new()),
        ("the caret moving down a row a frame", Vec::new()),
    ];
    for (pass, (_, times)) in passes.iter_mut().enumerate() {
        for frame in 0..FRAMES {
            let drawn = cx.update_window(handle, |_, window, cx| {
                state.update(cx, |state, cx| match pass {
                    0 => state.insert_text("a", cx),
                    _ => state.move_by_block(true, false, cx),
                });
                let started = Instant::now();
                window.draw(cx).clear(cx);
                started.elapsed()
            });
            let Ok(elapsed) = drawn else { return ExitCode::FAILURE };
            if frame >= 10 {
                times.push(elapsed);
            }
        }
    }
    let drawn = cx.update(|cx| editor.read(cx).drawn_leaves());
    for (label, times) in &mut passes {
        report(label, times, drawn);
    }
    ExitCode::SUCCESS
}
