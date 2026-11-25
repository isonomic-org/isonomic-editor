use crate::{BlockId, ColumnId, RowId};

/// Which side a position leans to when it sits on a wrap or run edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Affinity {
    Before,
    #[default]
    After,
}

/// A place in a block's text. Offsets are UTF-8 bytes on grapheme
/// boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Position {
    pub block: BlockId,
    pub offset: usize,
    pub affinity: Affinity,
}

impl Position {
    pub fn new(block: BlockId, offset: usize) -> Self {
        Self { block, offset, affinity: Affinity::After }
    }
}

/// What is selected in a document view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// A text range, which may span blocks. Collapsed when both ends
    /// are equal.
    Text { anchor: Position, head: Position },
    /// Whole blocks.
    Blocks(Vec<BlockId>),
    /// Table cells, by ID so the selection survives reordering.
    Cells { table: BlockId, rows: Vec<RowId>, columns: Vec<ColumnId> },
}

impl Selection {
    pub fn caret(block: BlockId, offset: usize) -> Self {
        let at = Position::new(block, offset);
        Self::Text { anchor: at, head: at }
    }

    pub fn range(block: BlockId, start: usize, end: usize) -> Self {
        Self::Text { anchor: Position::new(block, start), head: Position::new(block, end) }
    }

    pub fn is_caret(&self) -> bool {
        matches!(self, Self::Text { anchor, head }
            if anchor.block == head.block && anchor.offset == head.offset)
    }

    /// The moving end of a text selection.
    pub fn head(&self) -> Option<Position> {
        match self {
            Self::Text { head, .. } => Some(*head),
            _ => None,
        }
    }
}
