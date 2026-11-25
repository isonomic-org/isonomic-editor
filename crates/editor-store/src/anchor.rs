use editor_model::{Affinity, BlockId};
use loro::cursor::Cursor;

/// A position that survives edits, local and remote. Selection ends,
/// comment ranges and remote carets are held as anchors, never as raw
/// offsets.
#[derive(Clone, Debug)]
pub struct Anchor {
    pub(crate) block: BlockId,
    pub(crate) cursor: Option<Cursor>,
    pub(crate) affinity: Affinity,
}

impl Anchor {
    pub fn block(&self) -> BlockId {
        self.block
    }
}
