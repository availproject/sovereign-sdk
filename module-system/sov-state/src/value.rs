use std::marker::PhantomData;

use borsh::{BorshDeserialize, BorshSerialize};
use thiserror::Error;

use crate::codec::{BorshCodec, StateValueCodec};
use crate::{Prefix, Storage, WorkingSet};

/// Container for a single value.
#[derive(Debug, PartialEq, Eq, Clone, BorshDeserialize, BorshSerialize)]
pub struct StateValue<V, VC = BorshCodec> {
    _phantom: PhantomData<V>,
    codec: VC,
    prefix: Prefix,
}

/// Error type for `StateValue` get method.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Value not found for prefix: {0}")]
    MissingValue(Prefix),
}

impl<V> StateValue<V> {
    /// Crates a new [`StateValue`] with the given prefix and the default
    /// [`StateValueCodec`] (i.e. [`BorshCodec`]).
    pub fn new(prefix: Prefix) -> Self {
        Self::with_codec(prefix, BorshCodec)
    }
}

impl<V, VC> StateValue<V, VC> {
    /// Creates a new [`StateValue`] with the given prefix and codec.
    pub fn with_codec(prefix: Prefix, codec: VC) -> Self {
        Self {
            _phantom: PhantomData,
            codec,
            prefix,
        }
    }

    /// Returns the prefix used when this [`StateValue`] was created.
    pub fn prefix(&self) -> &Prefix {
        &self.prefix
    }
}

impl<V, VC> StateValue<V, VC>
where
    VC: StateValueCodec<V>,
{
    /// Sets a value in the StateValue.
    pub fn set<S: Storage>(&self, value: &V, working_set: &mut WorkingSet<S>) {
        working_set.set_value(self.prefix(), &SingletonKey, value, &self.codec)
    }

    /// Gets a value from the StateValue or None if the value is absent.
    pub fn get<S: Storage>(&self, working_set: &mut WorkingSet<S>) -> Option<V> {
        working_set.get_value(self.prefix(), &SingletonKey, &self.codec)
    }

    /// Gets a value from the StateValue or Error if the value is absent.
    pub fn get_or_err<S: Storage>(&self, working_set: &mut WorkingSet<S>) -> Result<V, Error> {
        self.get(working_set)
            .ok_or_else(|| Error::MissingValue(self.prefix().clone()))
    }

    /// Removes a value from the StateValue, returning the value (or None if the key is absent).
    pub fn remove<S: Storage>(&self, working_set: &mut WorkingSet<S>) -> Option<V> {
        working_set.remove_value(self.prefix(), &SingletonKey, &self.codec)
    }

    /// Removes a value and from the StateValue, returning the value (or Error if the key is absent).
    pub fn remove_or_err<S: Storage>(&self, working_set: &mut WorkingSet<S>) -> Result<V, Error> {
        self.remove(working_set)
            .ok_or_else(|| Error::MissingValue(self.prefix().clone()))
    }

    /// Deletes a value from the StateValue.
    pub fn delete<S: Storage>(&self, working_set: &mut WorkingSet<S>) {
        working_set.delete_value(self.prefix(), &SingletonKey);
    }
}

// SingletonKey is very similar to the unit type `()` i.e. it has only one value.
#[derive(Debug, PartialEq, Eq, Hash)]
struct SingletonKey;
