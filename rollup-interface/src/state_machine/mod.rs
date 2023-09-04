//! Defines types, traits, and helpers that are used by the core state-machine of the rollup.
//! Items in this module must be fully deterministic, since they are expected to be executed inside of zkVMs.
pub mod crypto;
pub mod da;
pub mod stf;
pub mod zk;

pub use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::de::DeserializeOwned;
use serde::Serialize;

#[cfg(feature = "mocks")]
pub mod mocks;

pub mod optimistic;

/// A marker trait for general addresses.
pub trait BasicAddress:
    Eq
    + PartialEq
    + core::fmt::Debug
    + core::fmt::Display
    + Send
    + Sync
    + Clone
    + std::hash::Hash
    + AsRef<[u8]>
    + for<'a> TryFrom<&'a [u8], Error = anyhow::Error>
    + std::str::FromStr
    + Serialize
    + DeserializeOwned
    + 'static
{
}

/// An address used inside rollup
pub trait RollupAddress: BasicAddress + From<[u8; 32]> {}
