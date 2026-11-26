// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Links: the bar that asks where selected text should point, and
//! opening a link from the text.

use std::ops::Range;

use editor_commands as cmd;
use editor_model::{BlockId, Content, Position, Selection, word_range};
use ui::base::input::{InputEvent, InputState};
use ui::component::ActiveTheme as _;
use ui::component::input::Input;
use ui::{
    AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Styled as _, Window, div,
};

use crate::actions::{Cancel, LINK_CONTEXT};
use crate::{EditorState, EditorView};

/// The address a link opens, if it is one this editor will open: a
/// web or mail address. A bare domain is taken to be a web address.
pub fn openable(url: &str) -> Option<String> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if ["https://", "http://", "mailto:"].iter().any(|scheme| lower.starts_with(scheme)) {
        return Some(url.to_owned());
    }
    let bare = !url.is_empty()
        && !url.contains(|c: char| c.is_whitespace() || c == ':')
        && url.split('/').next().is_some_and(|host| host.contains('.') && !host.starts_with('.'));
    bare.then(|| format!("https://{url}"))
}

impl EditorState {
    /// The address of the link at a place in the text.
    pub fn link_at(&self, at: Position) -> Option<String> {
        let doc = self.snapshot();
        match &doc.get(at.block)?.content {
            Content::Inline(text) => self.link_span(text, at.offset).map(|(_, url)| url),
            _ => None,
        }
    }

    /// The linked stretch around an offset and where it points. An
    /// offset at the end of a link counts as in it.
    fn link_span(
        &self,
        text: &editor_model::InlineText,
        offset: usize,
    ) -> Option<(Range<usize>, String)> {
        let mut found: Option<(Range<usize>, String)> = None;
        for (range, run) in text.spans() {
            match (&run.marks.link, &mut found) {
                // The same link goes on through runs that differ in other marks.
                (Some(url), Some((span, held))) if url == held && span.end == range.start => {
                    span.end = range.end;
                }
                (link, _) => {
                    if found
                        .as_ref()
                        .is_some_and(|(span, _)| span.contains(&offset) || span.end == offset)
                    {
                        break;
                    }
                    found = link.clone().map(|url| (range, url));
                }
            }
        }
        found.filter(|(span, _)| span.contains(&offset) || span.end == offset)
    }

    /// What a link would be put on, and the address it has now: the
    /// selection; or, for a caret, the link it is in, else the word.
    pub(crate) fn link_target(&self) -> Option<(Selection, Option<String>)> {
        let doc = self.snapshot();
        if !self.selection.is_caret() {
            let url = self.head().and_then(|head| self.link_at(head));
            return Some((self.selection.clone(), url));
        }
        let head = self.head()?;
        let Content::Inline(text) = &doc.get(head.block)?.content else { return None };
        let range =
            |block: BlockId, range: Range<usize>| Selection::range(block, range.start, range.end);
        if let Some((span, url)) = self.link_span(text, head.offset) {
            return Some((range(head.block, span), Some(url)));
        }
        // Punctuation and space on their own are not a word to link.
        let word = word_range(text.text(), head.offset);
        let wordy = text
            .text()
            .get(word.clone())
            .is_some_and(|word| word.chars().any(char::is_alphanumeric));
        wordy.then(|| (range(head.block, word), None))
    }

    /// Points `target` at `url`, or takes its link away for an empty
    /// one. The linked text ends up selected.
    pub fn set_link(&mut self, target: &Selection, url: &str, cx: &mut Context<Self>) -> bool {
        let doc = self.snapshot();
        let url = url.trim();
        let tx = cmd::set_link(&doc, target, (!url.is_empty()).then_some(url));
        self.run(tx, cx)
    }
}

impl EditorView {
    /// Opens the link bar for the selection, the link under the caret
    /// or the word under it.
    pub(crate) fn open_link_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((target, url)) = self.state.read(cx).link_target() else { return };
        let input = match &self.link_input {
            Some(input) => input.clone(),
            None => {
                let input =
                    cx.new(|cx| InputState::new(window, cx).placeholder("Paste or type a link"));
                cx.subscribe_in(&input, window, Self::link_typed).detach();
                self.link_input = Some(input.clone());
                input
            }
        };
        // The address is selected, so typing or pasting replaces it.
        input.update(cx, |input, cx| {
            input.set_value(url.unwrap_or_default(), window, cx);
            input.select_all(window, cx);
        });
        self.state.update(cx, |state, cx| state.select(target.clone(), cx));
        self.linking = Some(target);
        input.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    pub(crate) fn close_link_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.linking = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn link_typed(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let InputEvent::PressEnter { .. } = event else { return };
        let Some(target) = self.linking.take() else { return };
        let url = input.read(cx).value().to_string();
        self.state.update(cx, |state, cx| state.set_link(&target, &url, cx));
        self.close_link_bar(window, cx);
    }

    /// The link bar, while it is open.
    pub(crate) fn link_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        self.linking.as_ref()?;
        let input = self.link_input.as_ref()?;
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
                .key_context(LINK_CONTEXT)
                .on_action(
                    cx.listener(|this, _: &Cancel, window, cx| this.close_link_bar(window, cx)),
                )
                .child(div().w_64().child(Input::new(input).appearance(false)))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Enter to link, empty to unlink"),
                ),
        )
    }
}
