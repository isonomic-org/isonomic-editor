// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The caret's blink. How fast it blinks, and whether it blinks at
//! all, is the theme's to say.

use ui::{Context, Task};

use crate::view::EditorView;

/// Whether the caret is in the shown half of its blink.
pub struct Blink {
    shown: bool,
    /// Bumped on every restart, so a timer from before does nothing.
    epoch: usize,
    task: Task<()>,
}

impl Default for Blink {
    fn default() -> Self {
        Self { shown: true, epoch: 0, task: Task::ready(()) }
    }
}

impl Blink {
    pub fn shown(&self) -> bool {
        self.shown
    }
}

impl EditorView {
    /// Whether the caret would be painted now, were the editor focused.
    pub fn caret_shown(&self) -> bool {
        self.blink.shown()
    }

    /// Shows the caret and starts its blink from the beginning. Call
    /// when the selection or the text changes.
    pub(crate) fn restart_blink(&mut self, cx: &mut Context<Self>) {
        self.blink.epoch = self.blink.epoch.wrapping_add(1);
        self.blink.shown = true;
        self.schedule_blink(cx);
    }

    fn schedule_blink(&mut self, cx: &mut Context<Self>) {
        self.blink.task = Task::ready(());
        let policy = cx.try_global::<ui::base::Theme>().map(|theme| theme.cursor_blink);
        let Some(interval) = policy.unwrap_or_default().interval() else { return };
        let epoch = self.blink.epoch;
        let timer = cx.background_executor().timer(interval);
        self.blink.task = cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |this, cx| {
                if this.blink.epoch == epoch {
                    this.blink.shown = !this.blink.shown;
                    cx.notify();
                    this.schedule_blink(cx);
                }
            });
        });
    }
}
