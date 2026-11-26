// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Whole blocks from the keyboard: moving them among their siblings
//! and turning them into another type.

use editor_commands as cmd;
use editor_model::{PropValue, Props, kinds};
use ui::{Context, Div, InteractiveElement as _, Stateful};

use crate::actions::{
    MoveBlockDown, MoveBlockUp, TurnIntoBulleted, TurnIntoCode, TurnIntoHeading1, TurnIntoHeading2,
    TurnIntoHeading3, TurnIntoNumbered, TurnIntoQuote, TurnIntoText, TurnIntoTodo,
};
use crate::{EditorState, EditorView};

impl EditorState {
    /// Moves the selected blocks, with what is nested in them, one
    /// place up or down among their siblings.
    pub fn move_blocks(&mut self, down: bool, cx: &mut Context<Self>) -> bool {
        let doc = self.snapshot();
        let tx = match down {
            true => cmd::move_down(&doc, &self.selection),
            false => cmd::move_up(&doc, &self.selection),
        };
        self.run(tx, cx)
    }

    /// Turns the selected blocks into another type. A heading takes
    /// its level; every other type takes none.
    pub fn turn_into(&mut self, kind: &str, level: Option<i64>, cx: &mut Context<Self>) -> bool {
        let doc = self.snapshot();
        let props: Props =
            level.map(|level| ("level".to_owned(), PropValue::Int(level))).into_iter().collect();
        let tx = cmd::convert_block(&doc, &self.selection, kind, &props);
        self.run(tx, cx)
    }
}

impl EditorView {
    /// Answers the block actions on the editor's element.
    pub(crate) fn block_actions(
        &self,
        element: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let turn = |kind: &'static str, level: Option<i64>| {
            move |this: &mut Self, cx: &mut Context<Self>| {
                this.state.update(cx, |state, cx| state.turn_into(kind, level, cx));
            }
        };
        let (text, quote, code) =
            (turn(kinds::PARAGRAPH, None), turn(kinds::QUOTE, None), turn(kinds::CODE, None));
        let (one, two, three) = (
            turn(kinds::HEADING, Some(1)),
            turn(kinds::HEADING, Some(2)),
            turn(kinds::HEADING, Some(3)),
        );
        let (todo, bulleted, numbered) =
            (turn(kinds::TODO, None), turn(kinds::BULLETED, None), turn(kinds::NUMBERED, None));
        element
            .on_action(cx.listener(|this, _: &MoveBlockUp, _, cx| {
                this.state.update(cx, |state, cx| state.move_blocks(false, cx));
            }))
            .on_action(cx.listener(|this, _: &MoveBlockDown, _, cx| {
                this.state.update(cx, |state, cx| state.move_blocks(true, cx));
            }))
            .on_action(cx.listener(move |this, _: &TurnIntoText, _, cx| text(this, cx)))
            .on_action(cx.listener(move |this, _: &TurnIntoHeading1, _, cx| one(this, cx)))
            .on_action(cx.listener(move |this, _: &TurnIntoHeading2, _, cx| two(this, cx)))
            .on_action(cx.listener(move |this, _: &TurnIntoHeading3, _, cx| three(this, cx)))
            .on_action(cx.listener(move |this, _: &TurnIntoTodo, _, cx| todo(this, cx)))
            .on_action(cx.listener(move |this, _: &TurnIntoBulleted, _, cx| bulleted(this, cx)))
            .on_action(cx.listener(move |this, _: &TurnIntoNumbered, _, cx| numbered(this, cx)))
            .on_action(cx.listener(move |this, _: &TurnIntoQuote, _, cx| quote(this, cx)))
            .on_action(cx.listener(move |this, _: &TurnIntoCode, _, cx| code(this, cx)))
    }
}
