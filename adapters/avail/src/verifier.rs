use crate::spec::DaLayerSpec;
use serde::{Deserialize, Serialize};
use sov_rollup_interface::{
    da::{
        DaSpec, 
        DaVerifier
    },
    traits:: {
        BlockHeaderTrait, CanonicalHash, 
    },
    zk::traits::{ValidityCondition},
    crypto::{SimpleHasher}
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidityConditionError {
    #[error("conditions for validity can only be combined if the blocks are consecutive")]
    BlocksNotConsecutive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// A validity condition expressing that a chain of DA layer blocks is contiguous and canonical
pub struct ChainValidityCondition {
    pub prev_hash: [u8; 32],
    pub block_hash: [u8; 32],
}

impl ValidityCondition for ChainValidityCondition {
    type Error = ValidityConditionError;

    fn combine<SimpleHasher>(&self, rhs: Self) -> Result<Self, Self::Error> {
        if self.block_hash != rhs.prev_hash {
            return Err(ValidityConditionError::BlocksNotConsecutive);
        }

        Ok(rhs)
    }
}

pub struct Verifier;

impl DaVerifier for Verifier {
    type Spec = DaLayerSpec;

    type Error = ValidityConditionError;

    type ValidityCondition = ChainValidityCondition;

    // Verify that the given list of blob transactions is complete and correct.
    // NOTE: Function return unit since application client already verifies application data.
    fn verify_relevant_tx_list<SimpleHasher>(
        &self,
        _block_header: &<Self::Spec as DaSpec>::BlockHeader,
        _txs: &[<Self::Spec as DaSpec>::BlobTransaction],
        _inclusion_proof: <Self::Spec as DaSpec>::InclusionMultiProof,
        _completeness_proof: <Self::Spec as DaSpec>::CompletenessProof,
    ) -> Result<Self::ValidityCondition, Self::Error> {
        let validity_condition = ChainValidityCondition {
            prev_hash: *_block_header.prev_hash().inner(),
            block_hash: *_block_header.hash().inner(),
        };

        Ok(validity_condition)
    }

    fn new(_params: <Self::Spec as DaSpec>::ChainParams) -> Self {
        Verifier {}
    }
}
