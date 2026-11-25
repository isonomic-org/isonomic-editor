use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use editor_model::{
    ATOM_CHAR, BlockId, Changes, Content, Document, InlineText, Intent, Limits, ModelError,
    Normalizer, Op, Position, Registry, Selection, SeqIds, Transaction,
};
use loro::cursor::Side;
use loro::{
    ExpandType, ExportMode, LoroDoc, LoroTree, StyleConfig, StyleConfigMap, UndoManager,
    VersionVector,
};

use crate::anchor::Anchor;
use crate::convert::ATOM_KEY;
use crate::read::{self, Handles};
use crate::write;

const TREE: &str = "blocks";
const SYSTEM_ORIGIN: &str = "sys:normalize";
const MAX_UNDO_COMMITS: usize = 100_000;
const NORMALIZE_PASSES: usize = 4;

#[derive(Debug)]
pub enum StoreError {
    /// The model refused an operation; nothing was changed.
    Model(ModelError),
    /// The replication library refused an operation or an update.
    Replication(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(error) => write!(f, "{error}"),
            Self::Replication(error) => write!(f, "replication: {error}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<ModelError> for StoreError {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}

fn replication(error: impl fmt::Display) -> StoreError {
    StoreError::Replication(error.to_string())
}

/// The outcome of an undo or a redo.
#[derive(Clone, Debug, PartialEq)]
pub struct Undone {
    pub changes: Changes,
    /// The selection to restore.
    pub selection: Option<Selection>,
}

struct Step {
    commits: usize,
    intent: Intent,
    before: Option<Selection>,
    after: Option<Selection>,
}

/// One replicated document and its current snapshot.
pub struct Store {
    doc: LoroDoc,
    tree: LoroTree,
    undo: UndoManager,
    registry: Arc<Registry>,
    limits: Limits,
    snapshot: Document,
    handles: HashMap<BlockId, Handles>,
    ids: SeqIds,
    steps: Vec<Step>,
    undone: Vec<Step>,
    normalizer: Option<Normalizer>,
}

impl Store {
    /// An empty document. `replica` must be unique among the replicas
    /// that will ever edit this document.
    pub fn new(registry: Arc<Registry>, replica: u64) -> Self {
        Self::with_limits(registry, replica, Limits::default())
    }

    pub fn with_limits(registry: Arc<Registry>, replica: u64, limits: Limits) -> Self {
        let doc = LoroDoc::new();
        // A replica number the library rejects falls back to its own.
        let _ = doc.set_peer_id(replica);
        let mut styles = StyleConfigMap::new();
        for kind in editor_model::MarkKind::ALL {
            let expand = if kind.expands() { ExpandType::After } else { ExpandType::None };
            styles.insert(kind.key().into(), StyleConfig { expand });
        }
        styles.insert(ATOM_KEY.into(), StyleConfig { expand: ExpandType::None });
        doc.config_text_style(styles);
        let tree = doc.get_tree(TREE);
        tree.enable_fractional_index(0);
        let mut undo = UndoManager::new(&doc);
        undo.set_merge_interval(0);
        undo.set_max_undo_steps(MAX_UNDO_COMMITS);
        undo.add_exclude_origin_prefix("sys:");
        let snapshot = Document::with_limits(registry.clone(), limits.clone());
        Self {
            ids: SeqIds::new(doc.peer_id()),
            doc,
            tree,
            undo,
            registry,
            limits,
            snapshot,
            handles: HashMap::new(),
            steps: Vec::new(),
            undone: Vec::new(),
            normalizer: None,
        }
    }

    /// Opens a document from bytes written by [`Store::export_snapshot`]
    /// or [`Store::export_since`].
    pub fn open(registry: Arc<Registry>, replica: u64, bytes: &[u8]) -> Result<Self, StoreError> {
        let mut store = Self::new(registry, replica);
        store.doc.import(bytes).map_err(replication)?;
        store.rebuild();
        Ok(store)
    }

    pub fn set_normalizer(&mut self, normalizer: Normalizer) {
        self.normalizer = Some(normalizer);
    }

    /// The current snapshot. Cheap to clone and safe to keep.
    pub fn snapshot(&self) -> Document {
        self.snapshot.clone()
    }

    pub fn replica(&self) -> u64 {
        self.doc.peer_id()
    }

    /// The source of new block IDs for this replica.
    pub fn ids(&mut self) -> &mut SeqIds {
        &mut self.ids
    }

    /// Applies a transaction atomically: all of it, plus the repairs
    /// it calls for, or nothing. `before` is the selection the user
    /// had, restored by undo.
    pub fn apply(
        &mut self,
        tx: &Transaction,
        before: Option<&Selection>,
    ) -> Result<Changes, StoreError> {
        let mut changes = Changes::default();
        let mut dirty = false;
        let mut result = self.apply_ops(&tx.ops, &mut changes, &mut dirty);
        if result.is_ok() {
            result = self.normalize(&mut changes, &mut dirty);
        }
        if let Err(error) = result {
            if dirty {
                self.doc.commit();
                let _ = self.undo.undo();
                self.undone.clear();
                self.rebuild();
            }
            return Err(error);
        }
        if !dirty {
            return Ok(changes);
        }
        self.doc.commit();
        self.undone.clear();
        let after = tx.selection.clone();
        match self.steps.last_mut() {
            Some(last)
                if tx.intent == Intent::Typing
                    && last.intent == Intent::Typing
                    && last.after.is_some()
                    && last.after.as_ref() == before =>
            {
                last.commits += 1;
                last.after = after;
            }
            _ => self.steps.push(Step {
                commits: 1,
                intent: tx.intent,
                before: before.cloned(),
                after,
            }),
        }
        let total: usize = self.steps.iter().map(|step| step.commits).sum();
        if total > MAX_UNDO_COMMITS {
            self.steps.remove(0);
        }
        Ok(changes)
    }

    pub fn can_undo(&self) -> bool {
        !self.steps.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// Reverts this replica's last step. Other replicas' edits stay.
    pub fn undo(&mut self) -> Option<Undone> {
        let step = self.steps.pop()?;
        for _ in 0..step.commits {
            let _ = self.undo.undo();
        }
        let changes = self.rebuild();
        let selection = step.before.clone();
        self.undone.push(step);
        Some(Undone { changes, selection })
    }

    pub fn redo(&mut self) -> Option<Undone> {
        let step = self.undone.pop()?;
        for _ in 0..step.commits {
            let _ = self.undo.redo();
        }
        let changes = self.rebuild();
        let selection = step.after.clone();
        self.steps.push(step);
        Some(Undone { changes, selection })
    }

    /// The whole document with its history.
    pub fn export_snapshot(&self) -> Result<Vec<u8>, StoreError> {
        self.doc.export(ExportMode::Snapshot).map_err(replication)
    }

    /// An opaque version marker to hand to another replica.
    pub fn version(&self) -> Vec<u8> {
        self.doc.oplog_vv().encode()
    }

    /// Everything a replica at `version` is missing.
    pub fn export_since(&self, version: &[u8]) -> Result<Vec<u8>, StoreError> {
        let version = VersionVector::decode(version).map_err(replication)?;
        self.doc.export(ExportMode::updates(&version)).map_err(replication)
    }

    /// Merges bytes from another replica and reports what changed.
    pub fn import(&mut self, bytes: &[u8]) -> Result<Changes, StoreError> {
        self.doc.import(bytes).map_err(replication)?;
        let mut changes = self.rebuild();
        let mut dirty = false;
        self.doc.set_next_commit_origin(SYSTEM_ORIGIN);
        let repaired = self.normalize(&mut changes, &mut dirty);
        if dirty {
            self.doc.commit();
        }
        if repaired.is_err() {
            self.rebuild();
        }
        Ok(changes)
    }

    /// An anchor at a position, or `None` when the block is gone or
    /// has no text.
    pub fn anchor(&self, at: Position) -> Option<Anchor> {
        let block = self.snapshot.get(at.block)?;
        let text = block.content.text();
        let chars = text.get(..at.offset)?.chars().count();
        let cursor = self.handles.get(&at.block)?.text.get_cursor(chars, Side::Middle);
        Some(Anchor { block: at.block, cursor, affinity: at.affinity })
    }

    /// Where an anchor points now.
    pub fn resolve(&self, anchor: &Anchor) -> Option<Position> {
        let block = self.snapshot.get(anchor.block)?;
        let text = block.content.text();
        let chars = match &anchor.cursor {
            Some(cursor) => self.doc.get_cursor_pos(cursor).ok()?.current.pos,
            None => 0,
        };
        let offset = text.char_indices().nth(chars).map_or(text.len(), |(byte, _)| byte);
        Some(Position { block: anchor.block, offset, affinity: anchor.affinity })
    }

    fn apply_ops(
        &mut self,
        ops: &[Op],
        changes: &mut Changes,
        dirty: &mut bool,
    ) -> Result<(), StoreError> {
        for op in ops {
            let doomed = match op {
                Op::RemoveBlock { id } if self.snapshot.contains(*id) => self.snapshot.subtree(*id),
                _ => Vec::new(),
            };
            self.snapshot.apply(op)?;
            let id = op.target();
            let content = self.snapshot.get(id).map(|block| block.content.clone());
            let kept = content.as_ref().map(Content::to_inline);
            let plain;
            let written = match (op, &content) {
                (Op::ReplaceText { range, with, .. }, Some(Content::Plain(_))) => {
                    let text: String = with.text().chars().filter(|ch| *ch != ATOM_CHAR).collect();
                    plain = Op::ReplaceText {
                        id,
                        range: range.clone(),
                        with: InlineText::plain(&text),
                    };
                    &plain
                }
                _ => op,
            };
            *dirty = true;
            write::apply(&self.tree, &mut self.handles, written, kept).map_err(replication)?;
            for gone in doomed {
                self.handles.remove(&gone);
            }
            if matches!(op, Op::ReplaceText { .. } | Op::AddMark { .. } | Op::RemoveMark { .. })
                && let (Some(handles), Some(content)) = (self.handles.get(&id), &content)
            {
                let stored = read::text_of(&handles.text);
                if content.has_text() && stored != content.to_inline() {
                    self.snapshot.set_text(id, stored)?;
                }
            }
            changes.record(op);
        }
        Ok(())
    }

    fn normalize(&mut self, changes: &mut Changes, dirty: &mut bool) -> Result<(), StoreError> {
        let Some(mut normalizer) = self.normalizer.take() else {
            return Ok(());
        };
        let mut result = Ok(());
        for _ in 0..NORMALIZE_PASSES {
            let ops = normalizer(&self.snapshot, changes, &mut self.ids);
            if ops.is_empty() {
                break;
            }
            result = self.apply_ops(&ops, changes, dirty);
            if result.is_err() {
                break;
            }
        }
        self.normalizer = Some(normalizer);
        result
    }

    /// Re-reads the replicated document and reports what differs from
    /// the previous snapshot.
    fn rebuild(&mut self) -> Changes {
        let (snapshot, handles) = read::materialize(&self.tree, &self.registry, &self.limits);
        let changes = read::diff(&self.snapshot, &snapshot);
        for id in &changes.added {
            self.ids.observe(*id);
        }
        self.snapshot = snapshot;
        self.handles = handles;
        changes
    }
}
