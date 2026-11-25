// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The insert menu: typing the trigger in a block of text opens a list
//! of block types under the caret, filtered by what is typed next.

use editor_blocks::{MenuEntry, search_menu, standard};
use editor_commands as cmd;
use editor_model::Position;
use ui::base::text::editor_host::Caret;
use ui::component::{ActiveTheme as _, ThemeStyled as _};
use ui::{
    Anchor, AnyElement, Bounds, Context, InteractiveElement as _, IntoElement as _, MouseButton,
    ParentElement as _, Pixels, Styled as _, Window, anchored, canvas, deferred, div, point, rems,
};

use crate::state::EditorState;
use crate::view::EditorView;

/// The narrowest the menu is drawn, in rems.
const MENU_REMS: f32 = 14.;
/// The gap between the caret's line and the menu, in rems.
const GAP_REMS: f32 = 0.25;
/// The space above and below each row's text together, in rems.
const ROW_PADDING_REMS: f32 = 0.5;

/// The open insert menu.
#[derive(Clone, Debug, PartialEq)]
pub struct InsertMenu {
    /// Where the trigger character sits.
    pub from: Position,
    /// What matches the text typed after it, best first.
    pub entries: Vec<MenuEntry>,
    /// The entry Enter would choose.
    pub selected: usize,
}

impl EditorState {
    pub fn insert_menu(&self) -> Option<&InsertMenu> {
        self.menu.as_ref()
    }

    /// Call after text is typed: opens the menu if that text was the
    /// trigger in a place that takes it.
    pub(crate) fn open_menu_if_triggered(&mut self, typed: &str, cx: &mut Context<Self>) {
        if self.menu.is_some() || !typed.ends_with(cmd::MENU_TRIGGER) {
            return;
        }
        if let Some(from) = cmd::menu_trigger(&self.snapshot(), self.selection()) {
            let entries = search_menu(&standard(), "");
            self.menu = Some(InsertMenu { from, entries, selected: 0 });
            cx.notify();
        }
    }

    /// Call after any change: narrows the menu to what is now typed
    /// after the trigger, or closes it when that is nothing it knows.
    pub(crate) fn follow_menu(&mut self) {
        let Some(menu) = &mut self.menu else { return };
        let doc = self.store.snapshot();
        let entries = cmd::menu_query(&doc, &self.selection, menu.from)
            .map(|query| search_menu(&standard(), &query))
            .unwrap_or_default();
        if entries.is_empty() {
            self.menu = None;
        } else {
            menu.selected = menu.selected.min(entries.len() - 1);
            menu.entries = entries;
        }
    }

    /// Moves the menu's selection down or up, around the ends.
    pub fn step_menu(&mut self, down: bool, cx: &mut Context<Self>) {
        if let Some(menu) = &mut self.menu {
            let count = menu.entries.len().max(1);
            menu.selected = (menu.selected + if down { 1 } else { count - 1 }) % count;
            cx.notify();
        }
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            cx.notify();
        }
    }

    /// Applies an entry of the open menu, the selected one by default.
    pub fn choose_menu_entry(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else { return };
        let Some(entry) = menu.entries.get(index.unwrap_or(menu.selected)) else { return };
        let doc = self.snapshot();
        let tx = cmd::choose_menu_entry(
            &doc,
            &self.selection,
            menu.from,
            entry.kind,
            &entry.props,
            self.store.ids(),
        );
        self.run(tx, cx);
        cx.notify();
    }
}

impl EditorView {
    /// Where the open insert menu was drawn in the last frame.
    pub fn menu_bounds(&self) -> Option<Bounds<Pixels>> {
        self.menu_drawn.get()
    }

    /// The open menu as an element floating by the trigger. Nothing
    /// when the menu is closed or its block is not drawn.
    pub(crate) fn render_menu(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.menu_drawn.set(None);
        let drawn = self.menu_drawn.clone();
        let menu = self.state.read(cx).insert_menu()?.clone();
        let (over, under) = {
            let frame = self.frame.borrow();
            let leaf = frame.leaves.get(&self.keys.borrow().get(menu.from.block)?)?;
            let at = leaf.position_for_caret(Caret::new(menu.from.offset.min(leaf.len())));
            (at, at + point(ui::px(0.), leaf.caret_height()))
        };
        // Under the caret when it fits there, above it when it does not.
        let rows = (menu.entries.len() + 1) as f32;
        let tall = (window.line_height() + window.rem_size() * ROW_PADDING_REMS) * rows
            + window.rem_size() * ROW_PADDING_REMS * 2.;
        let below = under.y + tall <= window.viewport_size().height;
        let gap = point(ui::px(0.), window.rem_size() * GAP_REMS);
        let (corner, at) =
            if below { (Anchor::TopLeft, under + gap) } else { (Anchor::BottomLeft, over - gap) };
        let theme = cx.theme();
        let (active, muted) = (theme.list_active, theme.muted_foreground);
        let rows = menu.entries.iter().enumerate().map(|(index, entry)| {
            div()
                .id(("insert-menu-entry", index))
                .px_3()
                .py_1()
                .when_selected(index == menu.selected, active)
                .child(entry.title)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.state.update(cx, |state, cx| state.choose_menu_entry(Some(index), cx));
                    }),
                )
        });
        let list = div()
            .id("insert-menu")
            .occlude()
            .popover_style(cx)
            .min_w(rems(MENU_REMS))
            .py_1()
            .flex()
            .flex_col()
            .child(div().px_3().py_1().text_xs().text_color(muted).child("Insert"))
            .children(rows)
            .child(
                // Notes where the menu ended up, for tests and for hosts.
                canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            );
        let floating = anchored().anchor(corner).position(at).snap_to_window().child(list);
        Some(deferred(floating).into_any_element())
    }
}

/// A background for the selected row only.
trait WhenSelected: Sized {
    fn when_selected(self, selected: bool, color: ui::Hsla) -> Self;
}

impl<T: ui::Styled> WhenSelected for T {
    fn when_selected(self, selected: bool, color: ui::Hsla) -> Self {
        if selected { self.bg(color) } else { self }
    }
}
