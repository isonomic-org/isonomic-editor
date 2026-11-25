use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    Block, BlockDraft, BlockId, BlockKind, Content, ContentKind, InlineText, Limits, ModelError,
    Op, Registry,
};

#[derive(Clone)]
struct Node {
    block: Arc<Block>,
    parent: Option<BlockId>,
    rev: u64,
}

/// How many pieces the node table is split into. A snapshot shares
/// every piece with the one it was cloned from, and an edit copies
/// only the piece it touches, so a transaction costs the same whether
/// or not a reader is holding the previous snapshot.
const SHARDS: usize = 1024;

#[derive(Clone)]
struct Nodes {
    shards: Vec<Arc<HashMap<BlockId, Node>>>,
    len: usize,
}

impl Nodes {
    fn new() -> Self {
        Self { shards: (0..SHARDS).map(|_| Arc::default()).collect(), len: 0 }
    }

    fn shard(id: &BlockId) -> usize {
        let folded = (id.0 ^ (id.0 >> 64)) as u64;
        (folded.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 54) as usize % SHARDS
    }

    fn get(&self, id: &BlockId) -> Option<&Node> {
        self.shards[Self::shard(id)].get(id)
    }

    fn get_mut(&mut self, id: &BlockId) -> Option<&mut Node> {
        let shard = &mut self.shards[Self::shard(id)];
        if !shard.contains_key(id) {
            return None;
        }
        Arc::make_mut(shard).get_mut(id)
    }

    fn contains_key(&self, id: &BlockId) -> bool {
        self.get(id).is_some()
    }

    fn insert(&mut self, id: BlockId, node: Node) {
        if Arc::make_mut(&mut self.shards[Self::shard(&id)]).insert(id, node).is_none() {
            self.len += 1;
        }
    }

    fn remove(&mut self, id: &BlockId) -> Option<Node> {
        if !self.contains_key(id) {
            return None;
        }
        self.len -= 1;
        Arc::make_mut(&mut self.shards[Self::shard(id)]).remove(id)
    }
}

#[derive(Clone)]
struct Inner {
    registry: Arc<Registry>,
    limits: Limits,
    nodes: Nodes,
    root: Vec<BlockId>,
    revision: u64,
}

/// An immutable snapshot of a document: a tree of blocks under a root.
///
/// Cloning is cheap. Applying an operation to a clone leaves the
/// original untouched.
#[derive(Clone)]
pub struct Document {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("blocks", &self.len())
            .field("revision", &self.revision())
            .finish()
    }
}

impl Document {
    pub fn new(registry: Arc<Registry>) -> Self {
        Self::with_limits(registry, Limits::default())
    }

    pub fn with_limits(registry: Arc<Registry>, limits: Limits) -> Self {
        let inner = Inner { registry, limits, nodes: Nodes::new(), root: Vec::new(), revision: 0 };
        Self { inner: Arc::new(inner) }
    }

    pub fn registry(&self) -> &Arc<Registry> {
        &self.inner.registry
    }

    pub fn limits(&self) -> &Limits {
        &self.inner.limits
    }

    pub fn len(&self) -> usize {
        self.inner.nodes.len
    }

    pub fn is_empty(&self) -> bool {
        self.inner.nodes.len == 0
    }

    pub fn contains(&self, id: BlockId) -> bool {
        self.inner.nodes.contains_key(&id)
    }

    pub fn get(&self, id: BlockId) -> Option<&Block> {
        self.inner.nodes.get(&id).map(|node| &*node.block)
    }

    pub fn parent(&self, id: BlockId) -> Option<BlockId> {
        self.inner.nodes.get(&id).and_then(|node| node.parent)
    }

    /// Top-level blocks, in order.
    pub fn root(&self) -> &[BlockId] {
        &self.inner.root
    }

    /// The children of a block, or of the root for `None`.
    pub fn children(&self, parent: Option<BlockId>) -> &[BlockId] {
        match parent {
            None => &self.inner.root,
            Some(id) => self.get(id).map_or(&[], |block| &block.children),
        }
    }

    /// A counter that grows with every applied operation.
    pub fn revision(&self) -> u64 {
        self.inner.revision
    }

    /// The revision at which this block or anything under it last
    /// changed.
    pub fn rev(&self, id: BlockId) -> u64 {
        self.inner.nodes.get(&id).map_or(0, |node| node.rev)
    }

    pub fn index_in_parent(&self, id: BlockId) -> Option<usize> {
        let node = self.inner.nodes.get(&id)?;
        self.children(node.parent).iter().position(|child| *child == id)
    }

    /// How deep a block sits; top-level blocks are at depth 1.
    pub fn depth(&self, id: BlockId) -> usize {
        let mut depth = 0;
        let mut at = Some(id);
        while let Some(current) = at {
            if !self.contains(current) || depth > self.len() {
                break;
            }
            depth += 1;
            at = self.parent(current);
        }
        depth
    }

    /// The child indices from the root down to the block. Paths
    /// compare in document order.
    pub fn path(&self, id: BlockId) -> Option<Vec<usize>> {
        let mut path = Vec::new();
        let mut at = id;
        loop {
            path.push(self.index_in_parent(at)?);
            match self.parent(at) {
                Some(parent) => at = parent,
                None => break,
            }
        }
        path.reverse();
        Some(path)
    }

    pub fn is_ancestor(&self, ancestor: BlockId, of: BlockId) -> bool {
        let mut at = self.parent(of);
        while let Some(current) = at {
            if current == ancestor {
                return true;
            }
            at = self.parent(current);
        }
        false
    }

    /// Every block in document order (a parent before its children).
    pub fn flat(&self) -> Vec<BlockId> {
        let mut out = Vec::with_capacity(self.len());
        let mut stack: Vec<BlockId> = self.inner.root.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            out.push(id);
            stack.extend(self.children(Some(id)).iter().rev());
        }
        out
    }

    /// A block and everything under it, in document order.
    pub fn subtree(&self, id: BlockId) -> Vec<BlockId> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            out.push(current);
            stack.extend(self.children(Some(current)).iter().rev());
        }
        out
    }

    /// The block after this one in document order.
    pub fn next_in_order(&self, id: BlockId) -> Option<BlockId> {
        if let Some(first) = self.children(Some(id)).first() {
            return Some(*first);
        }
        let mut at = id;
        loop {
            let index = self.index_in_parent(at)?;
            let parent = self.parent(at);
            if let Some(next) = self.children(parent).get(index + 1) {
                return Some(*next);
            }
            at = parent?;
        }
    }

    /// The block before this one in document order.
    pub fn prev_in_order(&self, id: BlockId) -> Option<BlockId> {
        let index = self.index_in_parent(id)?;
        let parent = self.parent(id);
        if index == 0 {
            return parent;
        }
        let mut at = self.children(parent)[index - 1];
        while let Some(last) = self.children(Some(at)).last() {
            at = *last;
        }
        Some(at)
    }

    /// The operations that rebuild this document in an empty one.
    pub fn insert_ops(&self) -> Vec<Op> {
        let mut ops = Vec::with_capacity(self.len());
        for id in self.flat() {
            let Some(block) = self.get(id) else { continue };
            let draft = BlockDraft {
                id,
                kind: block.kind.clone(),
                props: block.props.clone(),
                tunes: block.tunes.clone(),
                text: block.content.to_inline(),
            };
            let index = self.index_in_parent(id).unwrap_or(0);
            ops.push(Op::InsertBlock { parent: self.parent(id), index, block: draft });
        }
        ops
    }

    /// Applies one operation. On error the document is unchanged.
    pub fn apply(&mut self, op: &Op) -> Result<(), ModelError> {
        self.check(op)?;
        let inner = Arc::make_mut(&mut self.inner);
        inner.revision += 1;
        match op {
            Op::InsertBlock { parent, index, block } => {
                let content = shape(&inner.registry, &block.kind, block.text.clone());
                let new = Block {
                    id: block.id,
                    kind: block.kind.clone(),
                    props: block.props.clone(),
                    tunes: block.tunes.clone(),
                    content,
                    children: Vec::new(),
                };
                inner
                    .nodes
                    .insert(block.id, Node { block: Arc::new(new), parent: *parent, rev: 0 });
                inner.children_mut(*parent).insert(*index, block.id);
                inner.touch(block.id);
            }
            Op::RemoveBlock { id } => {
                let parent = inner.nodes.get(id).and_then(|node| node.parent);
                inner.children_mut(parent).retain(|child| child != id);
                let mut stack = vec![*id];
                while let Some(current) = stack.pop() {
                    if let Some(node) = inner.nodes.remove(&current) {
                        stack.extend(&node.block.children);
                    }
                }
                inner.touch_parent(parent);
            }
            Op::MoveBlock { id, parent, index } => {
                let old = inner.nodes.get(id).and_then(|node| node.parent);
                inner.children_mut(old).retain(|child| child != id);
                inner.touch_parent(old);
                inner.children_mut(*parent).insert(*index, *id);
                if let Some(node) = inner.nodes.get_mut(id) {
                    node.parent = *parent;
                }
                inner.touch(*id);
            }
            Op::SetKind { id, kind } => {
                let registry = inner.registry.clone();
                let block = inner.block_mut(*id);
                let text = block.content.to_inline();
                block.content = shape(&registry, kind, text);
                block.kind = kind.clone();
                inner.touch(*id);
            }
            Op::SetProp { id, key, value } => {
                let props = &mut inner.block_mut(*id).props;
                match value {
                    Some(value) => props.insert(key.clone(), value.clone()),
                    None => props.remove(key),
                };
                inner.touch(*id);
            }
            Op::SetTune { id, key, value } => {
                let tunes = &mut inner.block_mut(*id).tunes;
                match value {
                    Some(value) => tunes.insert(key.clone(), value.clone()),
                    None => tunes.remove(key),
                };
                inner.touch(*id);
            }
            Op::ReplaceText { id, range, with } => {
                match &mut inner.block_mut(*id).content {
                    Content::Inline(inline) => inline.replace(range.clone(), with)?,
                    Content::Plain(text) => text.replace_range(range.clone(), &strip(with)),
                    Content::None => {}
                }
                inner.touch(*id);
            }
            Op::AddMark { id, range, mark } => {
                if let Content::Inline(inline) = &mut inner.block_mut(*id).content {
                    inline.add_mark(range.clone(), mark)?;
                }
                inner.touch(*id);
            }
            Op::RemoveMark { id, range, kind } => {
                if let Content::Inline(inline) = &mut inner.block_mut(*id).content {
                    inline.remove_mark(range.clone(), *kind)?;
                }
                inner.touch(*id);
            }
        }
        Ok(())
    }

    /// Overwrites a block's text with what a store read back. Meant
    /// for stores; editing goes through [`Document::apply`].
    pub fn set_text(&mut self, id: BlockId, text: InlineText) -> Result<(), ModelError> {
        if !self.contains(id) {
            return Err(ModelError::UnknownBlock(id));
        }
        let inner = Arc::make_mut(&mut self.inner);
        inner.revision += 1;
        let registry = inner.registry.clone();
        let block = inner.block_mut(id);
        block.content = shape(&registry, &block.kind.clone(), text);
        inner.touch(id);
        Ok(())
    }

    fn check(&self, op: &Op) -> Result<(), ModelError> {
        let known = |id: BlockId| {
            if self.contains(id) { Ok(()) } else { Err(ModelError::UnknownBlock(id)) }
        };
        let limits = &self.inner.limits;
        match op {
            Op::InsertBlock { parent, index, block } => {
                if self.contains(block.id) {
                    return Err(ModelError::DuplicateBlock(block.id));
                }
                if let Some(parent) = parent {
                    known(*parent)?;
                }
                if *index > self.children(*parent).len() {
                    return Err(ModelError::BadIndex);
                }
                if self.len() >= limits.blocks {
                    return Err(ModelError::Limit("blocks"));
                }
                if parent.map_or(0, |parent| self.depth(parent)) + 1 > limits.depth {
                    return Err(ModelError::Limit("depth"));
                }
                self.check_text(block.text.len(), block.text.runs().len())?;
                self.check_props(block.props.len().max(block.tunes.len()))
            }
            Op::RemoveBlock { id } => known(*id),
            Op::MoveBlock { id, parent, index } => {
                known(*id)?;
                if let Some(parent) = parent {
                    known(*parent)?;
                    if parent == id || self.is_ancestor(*id, *parent) {
                        return Err(ModelError::Cycle);
                    }
                }
                let same = self.parent(*id) == *parent;
                let len = self.children(*parent).len() - usize::from(same);
                if *index > len {
                    return Err(ModelError::BadIndex);
                }
                let height = self.subtree(*id).iter().map(|b| self.depth(*b)).max().unwrap_or(1)
                    - self.depth(*id);
                if parent.map_or(0, |parent| self.depth(parent)) + 1 + height > limits.depth {
                    return Err(ModelError::Limit("depth"));
                }
                Ok(())
            }
            Op::SetKind { id, .. } => known(*id),
            Op::SetProp { id, value, .. } | Op::SetTune { id, value, .. } => {
                known(*id)?;
                let block = self.get(*id).ok_or(ModelError::UnknownBlock(*id))?;
                let grows = usize::from(value.is_some());
                self.check_props(block.props.len().max(block.tunes.len()) + grows)
            }
            Op::ReplaceText { id, range, with } => {
                let content = &self.get(*id).ok_or(ModelError::UnknownBlock(*id))?.content;
                check_range(content, range.start, range.end)?;
                let runs = content.to_inline().runs().len() + with.runs().len() + 1;
                self.check_text(content.len() - (range.end - range.start) + with.len(), runs)
            }
            Op::AddMark { id, range, .. } | Op::RemoveMark { id, range, .. } => {
                let content = &self.get(*id).ok_or(ModelError::UnknownBlock(*id))?.content;
                if !matches!(content, Content::Inline(_)) {
                    return Err(ModelError::NoText);
                }
                check_range(content, range.start, range.end)?;
                self.check_text(content.len(), content.to_inline().runs().len() + 2)
            }
        }
    }

    fn check_text(&self, bytes: usize, runs: usize) -> Result<(), ModelError> {
        if bytes > self.inner.limits.text_bytes {
            return Err(ModelError::Limit("text per block"));
        }
        if runs > self.inner.limits.runs {
            return Err(ModelError::Limit("marks per block"));
        }
        Ok(())
    }

    fn check_props(&self, count: usize) -> Result<(), ModelError> {
        if count > self.inner.limits.props {
            Err(ModelError::Limit("props per block"))
        } else {
            Ok(())
        }
    }
}

impl Inner {
    fn children_mut(&mut self, parent: Option<BlockId>) -> &mut Vec<BlockId> {
        match parent {
            Some(id) if self.nodes.contains_key(&id) => &mut self.block_mut(id).children,
            _ => &mut self.root,
        }
    }

    /// Callers have checked that the block exists.
    fn block_mut(&mut self, id: BlockId) -> &mut Block {
        let node = self.nodes.get_mut(&id).expect("checked by Document::check");
        Arc::make_mut(&mut node.block)
    }

    fn touch_parent(&mut self, parent: Option<BlockId>) {
        if let Some(parent) = parent {
            self.touch(parent);
        }
    }

    /// Stamps the block and its ancestors with the current revision.
    fn touch(&mut self, id: BlockId) {
        let mut at = Some(id);
        while let Some(current) = at {
            let Some(node) = self.nodes.get_mut(&current) else {
                break;
            };
            if node.rev == self.revision && current != id {
                break;
            }
            node.rev = self.revision;
            at = node.parent;
        }
    }
}

fn check_range(content: &Content, start: usize, end: usize) -> Result<(), ModelError> {
    let text = match content {
        Content::None => return Err(ModelError::NoText),
        other => other.text(),
    };
    let ok = start <= end
        && end <= text.len()
        && text.is_char_boundary(start)
        && text.is_char_boundary(end);
    if ok { Ok(()) } else { Err(ModelError::BadRange) }
}

/// Plain text keeps no marks and no atoms.
fn strip(inline: &InlineText) -> String {
    inline.text().chars().filter(|ch| *ch != crate::ATOM_CHAR).collect()
}

fn shape(registry: &Registry, kind: &BlockKind, text: InlineText) -> Content {
    match registry.content(kind) {
        ContentKind::Inline => Content::Inline(text),
        ContentKind::Plain => Content::Plain(strip(&text)),
        ContentKind::None => Content::None,
    }
}
