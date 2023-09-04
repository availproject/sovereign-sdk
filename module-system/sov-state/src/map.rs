use std::borrow::Borrow;
use std::hash::Hash;
use std::marker::PhantomData;

use thiserror::Error;

use crate::codec::{BorshCodec, StateValueCodec};
use crate::storage::StorageKey;
use crate::{Prefix, Storage, WorkingSet};

/// A container that maps keys to values.
///
/// # Type parameters
/// [`StateMap`] is generic over:
/// - a key type `K`;
/// - a value type `V`;
/// - a [`StateValueCodec`] `VC`.
#[derive(Debug, Clone, PartialEq, borsh::BorshDeserialize, borsh::BorshSerialize)]
pub struct StateMap<K, V, VC = BorshCodec> {
    _phantom: (PhantomData<K>, PhantomData<V>),
    value_codec: VC,
    prefix: Prefix,
}

/// Error type for the [`StateMap::get`] method.
#[derive(Debug, Error)]
pub enum StateMapError {
    #[error("Value not found for prefix: {0} and: storage key {1}")]
    MissingValue(Prefix, StorageKey),
}

impl<K, V> StateMap<K, V> {
    /// Creates a new [`StateMap`] with the given prefix and the default
    /// [`StateValueCodec`] (i.e. [`BorshCodec`]).
    pub fn new(prefix: Prefix) -> Self {
        Self::with_codec(prefix, BorshCodec)
    }
}

impl<K, V, VC> StateMap<K, V, VC> {
    /// Creates a new [`StateMap`] with the given prefix and [`StateValueCodec`].
    pub fn with_codec(prefix: Prefix, codec: VC) -> Self {
        Self {
            _phantom: (PhantomData, PhantomData),
            value_codec: codec,
            prefix,
        }
    }

    /// Returns the prefix used when this [`StateMap`] was created.
    pub fn prefix(&self) -> &Prefix {
        &self.prefix
    }
}

impl<K, V, VC> StateMap<K, V, VC>
where
    K: Hash + Eq,
    VC: StateValueCodec<V>,
{
    /// Inserts a key-value pair into the map.
    ///
    /// Much like [`StateMap::get`], the key may be any borrowed form of the
    /// map’s key type.
    pub fn set<Q, S: Storage>(&self, key: &Q, value: &V, working_set: &mut WorkingSet<S>)
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        working_set.set_value(self.prefix(), key, value, &self.value_codec)
    }

    /// Returns the value corresponding to the key, or [`None`] if the map
    /// doesn't contain the key.
    ///
    /// # Examples
    ///
    /// The key may be any borrowed form of the map’s key type. Note that
    /// [`Hash`] and [`Eq`] on the borrowed form must match those for the key
    /// type.
    ///
    /// ```
    /// use sov_state::{StateMap, Storage, WorkingSet};
    ///
    /// fn foo<S>(map: StateMap<Vec<u8>, u64>, key: &[u8], ws: &mut WorkingSet<S>) -> Option<u64>
    /// where
    ///     S: Storage,
    /// {
    ///     // We perform the `get` with a slice, and not the `Vec`. it is so because `Vec` borrows
    ///     // `[T]`.
    ///     map.get(key, ws)
    /// }
    /// ```
    ///
    /// If the map's key type does not implement [`Borrow`] for your desired
    /// target type, you'll have to convert the key to something else. An
    /// example of this would be "slicing" an array to use in [`Vec`]-keyed
    /// maps:
    ///
    /// ```
    /// use sov_state::{StateMap, Storage, WorkingSet};
    ///
    /// fn foo<S>(map: StateMap<Vec<u8>, u64>, key: [u8; 32], ws: &mut WorkingSet<S>) -> Option<u64>
    /// where
    ///     S: Storage,
    /// {
    ///     map.get(&key[..], ws)
    /// }
    /// ```
    pub fn get<Q, S: Storage>(&self, key: &Q, working_set: &mut WorkingSet<S>) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        working_set.get_value(self.prefix(), key, &self.value_codec)
    }

    /// Returns the value corresponding to the key or [`StateMapError`] if key is absent in
    /// the map.
    pub fn get_or_err<Q, S: Storage>(
        &self,
        key: &Q,
        working_set: &mut WorkingSet<S>,
    ) -> Result<V, StateMapError>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(key, working_set).ok_or_else(|| {
            StateMapError::MissingValue(self.prefix().clone(), StorageKey::new(self.prefix(), key))
        })
    }

    /// Removes a key from the map, returning the corresponding value (or
    /// [`None`] if the key is absent).
    pub fn remove<Q, S: Storage>(&self, key: &Q, working_set: &mut WorkingSet<S>) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        working_set.remove_value(self.prefix(), key, &self.value_codec)
    }

    /// Removes a key from the map, returning the corresponding value (or
    /// [`StateMapError`] if the key is absent).
    ///
    /// Use [`StateMap::remove`] if you want an [`Option`] instead of a [`Result`].
    pub fn remove_or_err<Q, S: Storage>(
        &self,
        key: &Q,
        working_set: &mut WorkingSet<S>,
    ) -> Result<V, StateMapError>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.remove(key, working_set).ok_or_else(|| {
            StateMapError::MissingValue(self.prefix().clone(), StorageKey::new(self.prefix(), key))
        })
    }

    /// Deletes a key-value pair from the map.
    ///
    /// This is equivalent to [`StateMap::remove`], but doesn't deserialize and
    /// return the value beforing deletion.
    pub fn delete<Q, S: Storage>(&self, key: &Q, working_set: &mut WorkingSet<S>)
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        working_set.delete_value(self.prefix(), key);
    }
}

#[cfg(feature = "arbitrary")]
impl<'a, K, V, VC> StateMap<K, V, VC>
where
    K: arbitrary::Arbitrary<'a> + Hash + Eq,
    V: arbitrary::Arbitrary<'a> + Hash + Eq,
    VC: StateValueCodec<V> + Default,
{
    pub fn arbitrary_workset<S>(
        u: &mut arbitrary::Unstructured<'a>,
        working_set: &mut WorkingSet<S>,
    ) -> arbitrary::Result<Self>
    where
        S: Storage,
    {
        use arbitrary::Arbitrary;

        let prefix = Prefix::arbitrary(u)?;
        let len = u.arbitrary_len::<(K, V)>()?;
        let codec = VC::default();
        let map = StateMap::with_codec(prefix, codec);

        (0..len).try_fold(map, |map, _| {
            let key = K::arbitrary(u)?;
            let value = V::arbitrary(u)?;

            map.set(&key, &value, working_set);

            Ok(map)
        })
    }
}
