use std::fmt::Display;
use std::hash::Hash;
use std::sync::Arc;

use anyhow::ensure;
use borsh::{BorshDeserialize, BorshSerialize};
use hex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sov_first_read_last_write_cache::{CacheKey, CacheValue};

use crate::codec::StateValueCodec;
use crate::internal_cache::OrderedReadsAndWrites;
use crate::utils::AlignedVec;
use crate::witness::Witness;
use crate::{Prefix, StateMap};

// `Key` type for the `Storage`
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, BorshDeserialize, BorshSerialize)]
pub struct StorageKey {
    key: Arc<Vec<u8>>,
}

impl From<CacheKey> for StorageKey {
    fn from(cache_key: CacheKey) -> Self {
        Self { key: cache_key.key }
    }
}

impl StorageKey {
    pub fn key(&self) -> Arc<Vec<u8>> {
        self.key.clone()
    }

    pub fn to_cache_key(&self) -> CacheKey {
        CacheKey {
            key: self.key.clone(),
        }
    }

    pub fn into_cache_key(self) -> CacheKey {
        CacheKey { key: self.key }
    }
}

impl AsRef<Vec<u8>> for StorageKey {
    fn as_ref(&self) -> &Vec<u8> {
        &self.key
    }
}

impl Display for StorageKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:x?}", hex::encode(self.key().as_ref()))
    }
}

impl StorageKey {
    /// Creates a new StorageKey that combines a prefix and a key.
    pub fn new<K>(prefix: &Prefix, key: &K) -> Self
    where
        K: Hash + ?Sized,
    {
        let encoded_key = nohash_serialize(key);
        let encoded_key = AlignedVec::new(encoded_key);

        let full_key = Vec::<u8>::with_capacity(prefix.len() + encoded_key.len());
        let mut full_key = AlignedVec::new(full_key);
        full_key.extend(prefix.as_aligned_vec());
        full_key.extend(&encoded_key);

        Self {
            key: Arc::new(full_key.into_inner()),
        }
    }
}

// Serializes a value into a `Vec<u8>` using `std::hash::Hasher`
// writer methods, but without actually ever hashing anything.
fn nohash_serialize<T: Hash>(item: T) -> Vec<u8> {
    struct NoHasher(Vec<u8>);

    impl std::hash::Hasher for NoHasher {
        fn finish(&self) -> u64 {
            0
        }

        fn write(&mut self, bytes: &[u8]) {
            self.0.extend_from_slice(bytes);
        }
    }

    let mut hasher = NoHasher(vec![]);
    item.hash(&mut hasher);
    hasher.0
}

/// A serialized value suitable for storing. Internally uses an Arc<Vec<u8>> for cheap cloning.
#[derive(
    Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Default,
)]
pub struct StorageValue {
    value: Arc<Vec<u8>>,
}

impl From<CacheValue> for StorageValue {
    fn from(cache_value: CacheValue) -> Self {
        Self {
            value: cache_value.value,
        }
    }
}

impl From<Vec<u8>> for StorageValue {
    fn from(value: Vec<u8>) -> Self {
        Self {
            value: Arc::new(value),
        }
    }
}

impl StorageValue {
    /// Create a new storage value by serializing the input with the given codec.
    pub fn new<V, VC>(value: &V, codec: &VC) -> Self
    where
        VC: StateValueCodec<V>,
    {
        let encoded_value = codec.encode_value(value);
        Self {
            value: Arc::new(encoded_value),
        }
    }

    /// Get the bytes of this value.
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// Convert this value into a [`CacheValue`].
    pub fn into_cache_value(self) -> CacheValue {
        CacheValue { value: self.value }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshDeserialize, BorshSerialize)]
/// A proof that a particular storage key has a particular value, or is absent.
pub struct StorageProof<P> {
    /// The key which is proven
    pub key: StorageKey,
    /// The value, if any, which is proven
    pub value: Option<StorageValue>,
    /// The cryptographic proof
    pub proof: P,
}

/// An interface for storing and retrieving values in the storage.
pub trait Storage: Clone {
    /// The witness type for this storage instance.
    type Witness: Witness;

    /// The runtime config for this storage instance.
    type RuntimeConfig;

    /// A cryptographic proof that a particular key has a particular value, or is absent.
    type Proof: Serialize
        + DeserializeOwned
        + core::fmt::Debug
        + Clone
        + BorshSerialize
        + BorshDeserialize;

    /// State update that will be committed to the database.
    type StateUpdate;

    fn with_config(config: Self::RuntimeConfig) -> Result<Self, anyhow::Error>;

    /// Returns the value corresponding to the key or None if key is absent.
    fn get(&self, key: &StorageKey, witness: &Self::Witness) -> Option<StorageValue>;

    /// Returns the latest state root hash from the storage.
    fn get_state_root(&self, witness: &Self::Witness) -> anyhow::Result<[u8; 32]>;

    /// Calculates new state root but does not commit any changes to the database.
    fn compute_state_update(
        &self,
        state_accesses: OrderedReadsAndWrites,
        witness: &Self::Witness,
    ) -> Result<([u8; 32], Self::StateUpdate), anyhow::Error>;

    /// Commits state changes to the database.
    fn commit(&self, node_batch: &Self::StateUpdate);

    /// Validate all of the storage accesses in a particular cache log,
    /// returning the new state root after applying all writes.
    /// This function is equivalent to calling:
    /// `self.compute_state_update & self.commit`
    fn validate_and_commit(
        &self,
        state_accesses: OrderedReadsAndWrites,
        witness: &Self::Witness,
    ) -> Result<[u8; 32], anyhow::Error> {
        let (root_hash, node_batch) = self.compute_state_update(state_accesses, witness)?;
        self.commit(&node_batch);

        Ok(root_hash)
    }

    /// Opens a storage access proof and validates it against a state root.
    /// It returns a result with the opened leaf (key, value) pair in case of success.
    fn open_proof(
        &self,
        state_root: [u8; 32],
        proof: StorageProof<Self::Proof>,
    ) -> Result<(StorageKey, Option<StorageValue>), anyhow::Error>;

    fn verify_proof<K, V>(
        &self,
        state_root: [u8; 32],
        proof: StorageProof<Self::Proof>,
        expected_key: &K,
        storage_map: &StateMap<K, V>,
    ) -> Result<Option<StorageValue>, anyhow::Error>
    where
        K: Hash + Eq,
    {
        let (storage_key, storage_value) = self.open_proof(state_root, proof)?;

        // We have to check that the storage key is the same as the external key
        ensure!(
            storage_key == StorageKey::new(storage_map.prefix(), expected_key),
            "The storage key from the proof doesn't match the expected storage key."
        );

        Ok(storage_value)
    }

    /// Indicates if storage is empty or not.
    /// Useful during initialization
    fn is_empty(&self) -> bool;
}

// Used only in tests.
#[cfg(test)]
impl From<&'static str> for StorageKey {
    fn from(key: &'static str) -> Self {
        Self {
            key: Arc::new(key.as_bytes().to_vec()),
        }
    }
}

// Used only in tests.
#[cfg(test)]
impl From<&'static str> for StorageValue {
    fn from(value: &'static str) -> Self {
        Self {
            value: Arc::new(value.as_bytes().to_vec()),
        }
    }
}

pub trait NativeStorage: Storage {
    /// Returns the value corresponding to the key or None if key is absent and a proof to
    /// get the value. Panics if [`get_with_proof_opt`] returns `None` in place of the proof.
    fn get_with_proof(&self, key: StorageKey, witness: &Self::Witness)
        -> StorageProof<Self::Proof>;

    fn get_with_proof_from_state_map<K, V>(
        &self,
        key: &K,
        state_map: &StateMap<K, V>,
        witness: &Self::Witness,
    ) -> StorageProof<Self::Proof>
    where
        K: Hash + Eq,
    {
        self.get_with_proof(StorageKey::new(state_map.prefix(), key), witness)
    }
}
