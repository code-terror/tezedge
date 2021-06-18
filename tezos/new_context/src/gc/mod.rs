// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This sub module provides different KV alternatives for context persistence

use std::array::TryFromSliceError;
use std::collections::HashSet;
use std::sync::PoisonError;

use blake2::digest::InvalidOutputSize;
use failure::Fail;

use crypto::hash::{FromBytesError, HashType};

use crate::hash::HashingError;
use crate::persistent::{DBError, KeyValueStoreBackend};
use crate::working_tree::Entry;
use crate::{ContextKeyValueStoreSchema, EntryHash};

pub mod mark_move_gced;
pub mod mark_sweep_gced;

pub trait GarbageCollector {
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError>;

    fn block_applied(
        &mut self,
        referenced_older_entries: HashSet<EntryHash>,
    ) -> Result<(), GarbageCollectionError>;
}

pub trait NotGarbageCollected {}

impl<T: NotGarbageCollected> GarbageCollector for T {
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError> {
        Ok(())
    }

    fn block_applied(
        &mut self,
        _referenced_older_entries: HashSet<EntryHash>,
    ) -> Result<(), GarbageCollectionError> {
        Ok(())
    }
}

/// helper function for fetching and deserializing entry from the store
pub fn fetch_entry_from_store(
    store: &dyn KeyValueStoreBackend<ContextKeyValueStoreSchema>,
    hash: &EntryHash,
    path: &str,
) -> Result<Entry, GarbageCollectionError> {
    match store.get(&hash)? {
        None => Err(GarbageCollectionError::EntryNotFound {
            hash: HashType::ContextHash.hash_to_b58check(hash)?,
            path: path.to_string(),
        }),
        Some(entry_bytes) => Ok(bincode::deserialize(&entry_bytes)?),
    }
}

#[derive(Debug, Fail)]
pub enum GarbageCollectionError {
    #[fail(display = "Column family {} is missing", name)]
    MissingColumnFamily { name: &'static str },
    #[fail(display = "Backend Error")]
    BackendError,
    #[fail(display = "Guard Poison {} ", error)]
    GuardPoison { error: String },
    #[fail(display = "Serialization error: {:?}", error)]
    SerializationError { error: bincode::Error },
    #[fail(display = "DBError error: {:?}", error)]
    DBError { error: DBError },
    #[fail(display = "Failed to convert hash to array: {}", error)]
    HashConversionError { error: TryFromSliceError },
    #[fail(display = "GarbageCollector error: {}", error)]
    GarbageCollectorError { error: String },
    #[fail(display = "Mutex/lock lock error! Reason: {:?}", reason)]
    LockError { reason: String },
    #[fail(display = "Entry not found in store: path={:?} hash={:?}", path, hash)]
    EntryNotFound { hash: String, path: String },
    #[fail(display = "Failed to convert hash into string: {}", error)]
    HashToStringError { error: FromBytesError },
    #[fail(display = "Failed to encode hash: {}", error)]
    HashingError { error: HashingError },
    #[fail(display = "Invalid output size")]
    InvalidOutputSize,
    #[fail(display = "Expected value instead of `None` for {}", _0)]
    ValueExpected(&'static str),
}

impl From<DBError> for GarbageCollectionError {
    fn from(error: DBError) -> Self {
        GarbageCollectionError::DBError { error }
    }
}

impl From<bincode::Error> for GarbageCollectionError {
    fn from(error: bincode::Error) -> Self {
        GarbageCollectionError::SerializationError { error }
    }
}

impl From<TryFromSliceError> for GarbageCollectionError {
    fn from(error: TryFromSliceError) -> Self {
        GarbageCollectionError::HashConversionError { error }
    }
}

impl<T> From<PoisonError<T>> for GarbageCollectionError {
    fn from(pe: PoisonError<T>) -> Self {
        GarbageCollectionError::LockError {
            reason: format!("{}", pe),
        }
    }
}

impl From<FromBytesError> for GarbageCollectionError {
    fn from(error: FromBytesError) -> Self {
        GarbageCollectionError::HashToStringError { error }
    }
}

impl From<InvalidOutputSize> for GarbageCollectionError {
    fn from(_: InvalidOutputSize) -> Self {
        GarbageCollectionError::InvalidOutputSize
    }
}

impl From<HashingError> for GarbageCollectionError {
    fn from(error: HashingError) -> Self {
        Self::HashingError { error }
    }
}

impl slog::Value for GarbageCollectionError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}