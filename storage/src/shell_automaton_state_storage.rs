// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor};
use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;

use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::{default_table_options, RocksDbKeyValueSchema};
use crate::persistent::{BincodeEncoded, Decoder, Encoder, KeyValueSchema};
use crate::{Direction, IteratorMode, PersistentStorage, StorageError};

pub type ShellAutomatonStateIndexStorageKV =
    dyn TezedgeDatabaseWithIterator<ShellAutomatonStateStorage> + Sync + Send;

/// Storage for redux::State.
///
/// Indexed by ActionId that modified it [redux::State::last_action_id].
#[derive(Clone)]
pub struct ShellAutomatonStateStorage {
    kv: Arc<ShellAutomatonStateIndexStorageKV>,
}

impl ShellAutomatonStateStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.main_db(),
        }
    }

    #[inline]
    pub fn put<T>(&self, action_id: &u64, state_snapshot: &T) -> Result<(), StorageError>
    where
        T: Encoder,
    {
        self.kv
            .put(action_id, &state_snapshot.encode()?)
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn get<T>(&self, action_id: &u64) -> Result<Option<T>, StorageError>
    where
        T: Decoder,
    {
        let encoded = self.kv.get(action_id).map_err(StorageError::from)?;
        Ok(if let Some(encoded) = encoded {
            Some(T::decode(&encoded)?)
        } else {
            None
        })
    }

    /// Get closest state snapshot, where `state.last_action_id` <= `action_id`.
    #[inline]
    pub fn get_closest_before<T>(&self, action_id: &u64) -> Result<Option<T>, StorageError>
    where
        T: Decoder,
    {
        let results = self.kv.find(
            IteratorMode::From(action_id, Direction::Reverse),
            Some(1),
            Box::new(|_| Ok(true)),
        )?;

        Ok(results
            .get(0)
            .map(|(_, encoded)| T::decode(encoded))
            .transpose()?)
    }
}

impl KeyValueSchema for ShellAutomatonStateStorage {
    type Key = u64;
    type Value = Vec<u8>;
}

impl RocksDbKeyValueSchema for ShellAutomatonStateStorage {
    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "shell_automaton_state_storage"
    }
}

impl KVStoreKeyValueSchema for ShellAutomatonStateStorage {
    fn column_name() -> &'static str {
        Self::name()
    }
}