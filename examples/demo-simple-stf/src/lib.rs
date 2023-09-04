#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
use std::io::Read;
use std::marker::PhantomData;

use sha2::Digest;
use sov_rollup_interface::da::BlobReaderTrait;
use sov_rollup_interface::services::da::SlotData;
use sov_rollup_interface::stf::{BatchReceipt, SlotResult, StateTransitionFunction};
use sov_rollup_interface::zk::{ValidityCondition, Zkvm};

/// An implementation of the
/// [`StateTransitionFunction`](sov_rollup_interface::stf::StateTransitionFunction)
/// that is specifically designed to check if someone knows a preimage of a specific hash.
#[derive(PartialEq, Debug, Clone, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct CheckHashPreimageStf<Cond> {
    phantom_data: PhantomData<Cond>,
}

/// Outcome of the apply_slot method.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ApplySlotResult {
    /// Incorrect hash preimage was posted on the DA.
    Failure,
    /// Correct hash preimage was posted on the DA.
    Success,
}

impl<Vm: Zkvm, Cond: ValidityCondition, B: BlobReaderTrait> StateTransitionFunction<Vm, B>
    for CheckHashPreimageStf<Cond>
{
    // Since our rollup is stateless, we don't need to consider the StateRoot.
    type StateRoot = ();

    // This represents the initial configuration of the rollup, but it is not supported in this tutorial.
    type InitialState = ();

    // We could incorporate the concept of a transaction into the rollup, but we leave it as an exercise for the reader.
    type TxReceiptContents = ();

    // This is the type that will be returned as a result of `apply_blob`.
    type BatchReceiptContents = ApplySlotResult;

    // This data is produced during actual batch execution or validated with proof during verification.
    // However, in this tutorial, we won't use it.
    type Witness = ();

    type Condition = Cond;

    // Perform one-time initialization for the genesis block.
    fn init_chain(&mut self, _params: Self::InitialState) {
        // Do nothing
    }

    fn apply_slot<'a, I, Data>(
        &mut self,
        _witness: Self::Witness,
        _slot_data: &Data,
        blobs: I,
    ) -> SlotResult<
        Self::StateRoot,
        Self::BatchReceiptContents,
        Self::TxReceiptContents,
        Self::Witness,
    >
    where
        I: IntoIterator<Item = &'a mut B>,
        Data: SlotData,
    {
        let mut receipts = vec![];
        for blob in blobs {
            let blob_data = blob.data_mut();

            // Read the data from the blob as a byte vec.
            let mut data = Vec::new();

            // Panicking within the `StateTransitionFunction` is generally not recommended.
            // But here, if we encounter an error while reading the bytes,
            // it suggests a serious issue with the DA layer or our setup.
            blob_data
                .read_to_end(&mut data)
                .unwrap_or_else(|e| panic!("Unable to read blob data {}", e));

            // Check if the sender submitted the preimage of the hash.
            let hash = sha2::Sha256::digest(&data).into();
            let desired_hash = [
                102, 104, 122, 173, 248, 98, 189, 119, 108, 143, 193, 139, 142, 159, 142, 32, 8,
                151, 20, 133, 110, 226, 51, 179, 144, 42, 89, 29, 13, 95, 41, 37,
            ];

            let result = if hash == desired_hash {
                ApplySlotResult::Success
            } else {
                ApplySlotResult::Failure
            };

            // Return the `BatchReceipt`
            receipts.push(BatchReceipt {
                batch_hash: hash,
                tx_receipts: vec![],
                inner: result,
            });
        }

        SlotResult {
            state_root: (),
            batch_receipts: receipts,
            witness: (),
        }
    }

    fn get_current_state_root(&self) -> anyhow::Result<Self::StateRoot> {
        Ok(())
    }
}
