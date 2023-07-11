//! Defines the traits that must be implemented by zkVMs. A ZKVM like Risc0 consists of two components,
//! a "guest" and a "host". The guest is the zkVM program itself, and the host is the physical machine on
//! which the zkVM is running. Both the guest and the host are required to implement the [`Zkvm`] trait, in
//! addition to the specialized [`ZkvmGuest`] and [`ZkvmHost`] trait which is appropriate to that environment.
//!
//! For a detailed example showing how to implement these traits, see the
//! [risc0 adapter](https://github.com/Sovereign-Labs/sovereign-sdk/tree/main/adapters/risc0)
//! maintained by the Sovereign Labs team.
use core::fmt::Debug;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::crypto::SimpleHasher;

/// A trait implemented by the prover ("host") of a zkVM program.
pub trait ZkvmHost: Zkvm {
    /// Give the guest a piece of advice non-deterministically
    fn write_to_guest<T: Serialize>(&self, item: T);
}

/// A Zk proof system capable of proving and verifying arbitrary Rust code
/// Must support recursive proofs.
pub trait Zkvm {
    /// A commitment to the zkVM program which is being proven
    type CodeCommitment: Matches<Self::CodeCommitment>
        + Clone
        + Debug
        + Serialize
        + DeserializeOwned;

    /// The error type which is returned when a proof fails to verify
    type Error: Debug;

    /// Interpret a sequence of a bytes as a proof and attempt to verify it against the code commitment.
    /// If the proof is valid, return a reference to the public outputs of the proof.
    fn verify<'a>(
        serialized_proof: &'a [u8],
        code_commitment: &Self::CodeCommitment,
    ) -> Result<&'a [u8], Self::Error>;
}

/// A trait which is accessible from within a zkVM program.
pub trait ZkvmGuest: Zkvm {
    /// Obtain "advice" non-deterministically from the host
    fn read_from_host<T: DeserializeOwned>(&self) -> T;
    /// Add a public output to the zkVM proof
    fn commit<T: Serialize>(&self, item: &T);
}

/// This trait is implemented on the struct/enum which expresses the validity condition
pub trait ValidityCondition: Serialize + DeserializeOwned {
    /// The error type returned when two [`ValidityCondition`]s cannot be combined.
    type Error: Into<anyhow::Error>;
    /// Combine two conditions into one (typically run inside a recursive proof).
    /// Returns an error if the two conditions cannot be combined
    fn combine<H: SimpleHasher>(&self, rhs: Self) -> Result<Self, Self::Error>;
}

/// The public output of a SNARK proof in Sovereign, this struct makes a claim that
/// the state of the rollup has transitioned from `initial_state_root` to `final_state_root`
/// if and only if the condition `validity_condition` is satisfied.
///
/// The period of time covered by a state transition proof may be a single slot, or a range of slots on the DA layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateTransition<C> {
    /// The state of the rollup before the transition
    pub initial_state_root: [u8; 32],
    /// The state of the rollup after the transition
    pub final_state_root: [u8; 32],
    /// An additional validity condition for the state transition which needs
    /// to be checked outside of the zkVM circuit. This typically corresponds to
    /// some claim about the DA layer history, such as (X) is a valid block on the DA layer
    pub validity_condition: C,
}

/// This trait expresses that a type can check a validity condition.
pub trait ValidityConditionChecker<Condition: ValidityCondition> {
    /// The error type returned when a [`ValidityCondition`] is invalid.
    type Error: Into<anyhow::Error>;
    /// Check a validity condition
    fn check(&mut self, condition: &Condition) -> Result<(), Self::Error>;
}

/// A trait expressing that two items of a type are (potentially fuzzy) matches.
/// We need a custom trait instead of relying on [`PartialEq`] because we allow fuzzy matches.
pub trait Matches<T> {
    /// Check if two items are a match
    fn matches(&self, other: &T) -> bool;
}
