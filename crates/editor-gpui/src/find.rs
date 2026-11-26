// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finding text: the matches the state holds, and the bar the view
//! shows to type what to find.

use std::ops::Range;

use editor_commands as cmd;
use editor_model::{BlockId, Selection};
use ui::base::input::{InputEvent, InputState};
use ui::component::ActiveTheme as _;
use ui::component::input::Input;
use ui::{
    AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, Styled as _, Window, div,
};

use crate::actions::{Cancel, FIND_CONTEXT, FindNext, FindPrevious};
use crate::{EditorState, EditorView};

/// What is being looked for and where it was found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Find {
    query: String,
    matches: Vec<(BlockId, Range<usize>)>,
    current: Option<usize>,
}

impl Find {
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Every match, in document order.
    pub fn matches(&self) -> &[(BlockId, Range<usize>)] {
        &self.matches
    }

    /// The place among the matches of the one that is selected.
    pub fn current(&self) -> Option<usize> {
        self.current
    }
}

impl EditorState {
    /// What is being looked for, while the search is open.
    pub fn found(&self) -> Option<&Find> {
        self.find.as_ref()
    }

    /// Looks for `query`, ignoring case, and selects the first match
    /// at or after the selection. An empty query finds nothing and
    /// keeps the search open.
    pub fn find(&mut self, query: &str, cx: &mut Context<Self>) {
        let doc = self.snapshot();
        let matches = cmd::find_text(&doc, query);
        // The first match that does not start before the selection.
        let from = cmd::selection_ranges(&doc, &self.selection).into_iter().next();
        let order = doc.flat();
        let place = |id: BlockId| order.iter().position(|other| *other == id);
        let current = from
            .and_then(|(block, range)| {
                let at = (place(block)?, range.start);
                matches
                    .iter()
                    .position(|(id, found)| place(*id).is_some_and(|row| (row, found.start) >= at))
            })
            .or((!matches.is_empty()).then_some(0));
        self.find = Some(Find { query: query.to_owned(), matches, current });
        self.select_found(cx);
        cx.notify();
    }

    /// Selects the match after the current one, or the one before it,
    /// going round at the ends.
    pub fn find_next(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(find) = &mut self.find else { return };
        let count = find.matches.len();
        if count == 0 {
            return;
        }
        find.current = Some(match (find.current, forward) {
            (Some(at), true) => (at + 1) % count,
            (Some(at), false) => (at + count - 1) % count,
            (None, true) => 0,
            (None, false) => count - 1,
        });
        self.select_found(cx);
        cx.notify();
    }

    /// Closes the search. The selection stays on the last match.
    pub fn end_find(&mut self, cx: &mut Context<Self>) {
        if self.find.take().is_some() {
            cx.notify();
        }
    }

    /// Looks again after the document changed. The current match is
    /// the one the selection still covers, if there is one.
    pub(crate) fn refind(&mut self) {
        let Some(find) = &self.find else { return };
        let doc = self.snapshot();
        let matches = cmd::find_text(&doc, &find.query);
        let selected = cmd::selection_ranges(&doc, &self.selection);
        let current = match selected.as_slice() {
            [one] => matches.iter().position(|found| found == one),
            _ => None,
        };
        self.find = Some(Find { query: find.query.clone(), matches, current });
    }

    /// The selected text, when the selection is a range inside one
    /// block.
    pub(crate) fn selected_text(&self) -> Option<String> {
        let doc = self.snapshot();
        if !matches!(self.selection, Selection::Text { .. }) {
            return None;
        }
        match cmd::selection_ranges(&doc, &self.selection).as_slice() {
            [(block, range)] if !range.is_empty() => {
                doc.get(*block)?.content.text().get(range.clone()).map(str::to_owned)
            }
            _ => None,
        }
    }

    fn select_found(&mut self, cx: &mut Context<Self>) {
        let found = self.find.as_ref().and_then(|find| find.matches.get(find.current?)).cloned();
        if let Some((block, range)) = found {
            self.select(Selection::range(block, range.start, range.end), cx);
        }
    }
}

impl EditorView {
    /// Opens the search bar, or goes back to it, with the selected
    /// text in it when the selection lies within one block.
    pub(crate) fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = match &self.find_input {
            Some(input) => input.clone(),
            None => {
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("Find"));
                cx.subscribe_in(&input, window, Self::find_typed).detach();
                self.find_input = Some(input.clone());
                input
            }
        };
        let seed = self.state.read(cx).selected_text();
        let query = match seed {
            Some(seed) => seed,
            None => input.read(cx).value().to_string(),
        };
        input.update(cx, |input, cx| input.set_value(query.clone(), window, cx));
        self.state.update(cx, |state, cx| state.find(&query, cx));
        input.focus_handle(cx).focus(window, cx);
    }

    /// Closes the search bar and gives the keyboard back to the text.
    pub(crate) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.end_find(cx));
        window.focus(&self.focus, cx);
    }

    fn find_typed(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                let query = input.read(cx).value().to_string();
                self.state.update(cx, |state, cx| {
                    if state.found().is_some_and(|find| find.query() != query) {
                        state.find(&query, cx);
                    }
                });
            }
            InputEvent::PressEnter { shift, .. } => {
                self.state.update(cx, |state, cx| state.find_next(!shift, cx));
            }
            InputEvent::Focus | InputEvent::Blur => {}
        }
    }

    /// The search bar, while the search is open.
    pub(crate) fn find_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let find = self.state.read(cx).found()?;
        let input = self.find_input.as_ref()?;
        let count = match (find.current(), find.matches().len()) {
            (_, 0) if find.query().is_empty() => String::new(),
            (_, 0) => "No matches".to_owned(),
            (Some(at), total) => format!("{} of {total}", at + 1),
            (None, total) => format!("{total} found"),
        };
        let theme = cx.theme();
        Some(
            div()
                .absolute()
                .top_2()
                .right_4()
                .flex()
                .items_center()
                .gap_2()
                .p_1()
                .pr_3()
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover)
                .shadow_md()
                .occlude()
                .key_context(FIND_CONTEXT)
                .on_action(cx.listener(|this, _: &Cancel, window, cx| this.close_find(window, cx)))
                .on_action(cx.listener(|this, _: &FindNext, _, cx| {
                    this.state.update(cx, |state, cx| state.find_next(true, cx))
                }))
                .on_action(cx.listener(|this, _: &FindPrevious, _, cx| {
                    this.state.update(cx, |state, cx| state.find_next(false, cx))
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        this.close_find(window, cx);
                        cx.stop_propagation();
                    }
                }))
                .child(div().w_48().child(Input::new(input).appearance(false)))
                .child(div().text_sm().text_color(theme.muted_foreground).child(count)),
        )
    }
}
