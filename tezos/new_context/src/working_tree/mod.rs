// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    rc::Rc,
};

use serde::{Deserialize, Serialize};

use crate::hash::{hash_entry, EntryHash, HashingError};
use crate::ContextValue;

use self::working_tree::MerkleError;

pub mod working_tree;
pub mod working_tree_stats; // TODO - TE-261 remove or reimplement

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
pub struct KeyFragment(pub(crate) Rc<str>);

impl std::ops::Deref for KeyFragment {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for KeyFragment {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<&str> for KeyFragment {
    fn from(value: &str) -> Self {
        KeyFragment(Rc::from(value))
    }
}

impl From<Rc<str>> for KeyFragment {
    fn from(value: Rc<str>) -> Self {
        KeyFragment(value)
    }
}

// Tree must be an ordered structure for consistent hash in hash_tree.
// The entry names *must* be in lexicographical order, as required by the hashing algorithm.
// Currently immutable OrdMap is used to allow cloning trees without too much overhead.
pub type Tree = im_rc::OrdMap<KeyFragment, Rc<Node>>;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum NodeKind {
    NonLeaf,
    Leaf,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Node {
    pub node_kind: NodeKind,
    /// True when the entry has already been commited.
    /// We don't need to serialize it twice.
    #[serde(skip)]
    #[serde(default = "node_serialized")]
    pub commited: Cell<bool>,
    #[serde(serialize_with = "ensure_non_null_entry_hash")]
    pub entry_hash: RefCell<Option<EntryHash>>,
    #[serde(skip)]
    pub entry: RefCell<Option<Entry>>,
}

fn node_serialized() -> Cell<bool> {
    // Deserializing the Node means it was already serialized
    Cell::new(true)
}

#[derive(Debug, Hash, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Commit {
    pub(crate) parent_commit_hash: Option<EntryHash>,
    pub(crate) root_hash: EntryHash,
    pub(crate) time: u64,
    pub(crate) author: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Entry {
    Tree(Tree),
    Blob(ContextValue),
    Commit(Commit),
}

impl Node {
    pub fn entry_hash(&self) -> Result<EntryHash, HashingError> {
        match &mut *self
            .entry_hash
            .try_borrow_mut()
            .map_err(|_| HashingError::EntryBorrow)?
        {
            Some(hash) => Ok(*hash),
            entry_hash @ None => {
                let hash = hash_entry(
                    self.entry
                        .try_borrow()
                        .map_err(|_| HashingError::EntryBorrow)?
                        .as_ref()
                        .ok_or(HashingError::MissingEntry)?,
                )?;
                entry_hash.replace(hash);
                Ok(hash)
            }
        }
    }

    pub fn get_hash(&self) -> Result<EntryHash, MerkleError> {
        self.entry_hash
            .try_borrow()
            .map_err(|_| MerkleError::InvalidState("The Entry hash is borrowed more than once"))?
            .as_ref()
            .copied()
            .ok_or(MerkleError::InvalidState("Missing entry hash"))
    }

    pub fn set_entry(&self, entry: &Entry) -> Result<(), MerkleError> {
        self.entry
            .try_borrow_mut()
            .map_err(|_| MerkleError::InvalidState("The Entry is borrowed more than once"))?
            .replace(entry.clone());

        Ok(())
    }
}

// Make sure the node contains the entry hash when serializing
fn ensure_non_null_entry_hash<S>(
    entry_hash: &RefCell<Option<EntryHash>>,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let entry_hash_ref = entry_hash.borrow();
    let entry_hash = entry_hash_ref
        .as_ref()
        .ok_or_else(|| serde::ser::Error::custom("entry_hash missing in Node"))?;

    s.serialize_some(entry_hash)
}