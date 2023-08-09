// TODO: Rename this file to change the name of this method from METHOD_NAME

#![no_main]
use demo_stf::app::create_zk_app_template;
use demo_stf::ArrayWitness;
use risc0_adapter::guest::Risc0Guest;
use risc0_zkvm::guest::env;
use sov_rollup_interface::crypto::NoOpHasher;
use sov_rollup_interface::da::{DaSpec, DaVerifier};
use sov_rollup_interface::stf::StateTransitionFunction;
use sov_rollup_interface::zk::{StateTransition, ZkvmGuest};
use presence::spec::{DaLayerSpec};
use presence::spec::header::AvailHeader;
use presence::verifier::Verifier;
use presence::spec::transaction::AvailBlobTransaction;

risc0_zkvm::guest::entry!(main);
// steps:
//  0. Read tx list and proofs
//  1. Call verify_relevant_tx_list()
//  2. Call begin_slot()
//  3. Decode each batch.
//  4. Call apply_batch
//  5. Call end_slot
//  6. Output (Da hash, start_root, end_root, event_root)
pub fn main() {
    env::write(&"Start guest\n");
    let guest = Risc0Guest;
    
    let prev_state_root_hash: [u8; 32] = guest.read_from_host();
    env::write(&"Prev root hash read\n");

    // Step 1: read tx list
    let header: AvailHeader = guest.read_from_host();
    env::write(&"header has been read\n");
    let inclusion_proof: <DaLayerSpec as DaSpec>::InclusionMultiProof = guest.read_from_host();
    env::write(&"inclusion proof has been read\n");
    let completeness_proof: <DaLayerSpec as DaSpec>::CompletenessProof = guest.read_from_host();
    env::write(&"completeness proof has been read\n");
    let mut blobs: Vec<AvailBlobTransaction> = guest.read_from_host();
    env::write(&"blobs have been read\n");

    // Step 2: Verify tx list
    let mut app = create_zk_app_template::<Risc0Guest, AvailBlobTransaction>(prev_state_root_hash);

    let witness: ArrayWitness = guest.read_from_host();
    env::write(&"Witness have been read\n");

    env::write(&"Applying slot...\n");
    let result = app.apply_slot(witness, &mut blobs);

    env::write(&"Slot has been applied\n");

    // Step 3: Verify tx list
    let verifier = presence::verifier::Verifier {};
    let validity_condition = verifier
    .verify_relevant_tx_list::<NoOpHasher>(&header, &blobs, inclusion_proof, completeness_proof)
    .expect("Transaction list must be correct");
    env::write(&"Relevant txs verified\n");

    let output = StateTransition {
        initial_state_root: prev_state_root_hash,
        final_state_root: result.state_root.0,
        validity_condition,
    };
    env::commit(&output);
    env::write(&"new state root committed\n");
}
