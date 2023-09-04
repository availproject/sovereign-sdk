use std::io::Read;

use sov_bank::TokenConfig;
use sov_blob_storage::BlobStorage;
use sov_modules_api::capabilities::{BlobRefOrOwned, BlobSelector};
use sov_modules_api::default_context::DefaultContext;
use sov_modules_api::digest::Digest;
use sov_modules_api::{Address, Context, Module, Spec};
use sov_rollup_interface::da::BlobReaderTrait;
use sov_rollup_interface::mocks::{MockAddress, MockBlob, MockDaSpec};
use sov_sequencer_registry::{SequencerConfig, SequencerRegistry};
use sov_state::{ProverStorage, WorkingSet};

type C = DefaultContext;
type B = MockBlob;
type Da = MockDaSpec;

const PREFERRED_SEQUENCER_KEY: &str = "preferred";
const REGULAR_SEQUENCER_KEY: &str = "regular";
const LOCKED_AMOUNT: u64 = 200;

fn generate_address(key: &str) -> <C as Spec>::Address {
    let hash: [u8; 32] = <C as Spec>::Hasher::digest(key.as_bytes()).into();
    Address::from(hash)
}

fn get_bank_config(
    preferred_sequencer: <C as Spec>::Address,
    regular_sequencer: <C as Spec>::Address,
) -> sov_bank::BankConfig<C> {
    let token_config: TokenConfig<C> = TokenConfig {
        token_name: "InitialToken".to_owned(),
        address_and_balances: vec![
            (preferred_sequencer, LOCKED_AMOUNT * 3),
            (regular_sequencer, LOCKED_AMOUNT * 3),
        ],
        authorized_minters: vec![],
        salt: 9,
    };

    sov_bank::BankConfig {
        tokens: vec![token_config],
    }
}

#[test]
fn priority_sequencer_flow() {
    let tmpdir = tempfile::tempdir().unwrap();
    let mut working_set = WorkingSet::new(ProverStorage::with_path(tmpdir.path()).unwrap());

    let preferred_sequencer_da = MockAddress::from([10u8; 32]);
    let preferred_sequencer_rollup = generate_address(PREFERRED_SEQUENCER_KEY);
    let regular_sequencer_da = MockAddress::from([30u8; 32]);
    let regular_sequencer_rollup = generate_address(REGULAR_SEQUENCER_KEY);

    let bank_config = get_bank_config(preferred_sequencer_rollup, regular_sequencer_rollup);

    let token_address = sov_bank::get_genesis_token_address::<C>(
        &bank_config.tokens[0].token_name,
        bank_config.tokens[0].salt,
    );

    let sequencer_registry_config = SequencerConfig {
        seq_rollup_address: preferred_sequencer_rollup,
        seq_da_address: preferred_sequencer_da.as_ref().to_vec(),
        coins_to_lock: sov_bank::Coins {
            amount: LOCKED_AMOUNT,
            token_address,
        },
        is_preferred_sequencer: true,
    };

    let bank = sov_bank::Bank::<C>::default();
    let sequencer_registry = SequencerRegistry::<C>::default();
    let blob_storage = BlobStorage::<C>::default();

    bank.genesis(&bank_config, &mut working_set).unwrap();
    sequencer_registry
        .genesis(&sequencer_registry_config, &mut working_set)
        .unwrap();
    blob_storage.genesis(&(), &mut working_set).unwrap();

    let register_message = sov_sequencer_registry::CallMessage::Register {
        da_address: regular_sequencer_da.as_ref().to_vec(),
    };
    sequencer_registry
        .call(
            register_message,
            &C::new(regular_sequencer_rollup),
            &mut working_set,
        )
        .unwrap();

    let blob_1 = B::new(vec![1], regular_sequencer_da, [1u8; 32]);
    let blob_2 = B::new(vec![2, 2], regular_sequencer_da, [2u8; 32]);
    let blob_3 = B::new(vec![3, 3, 3], preferred_sequencer_da, [3u8; 32]);
    let blob_4 = B::new(vec![4, 4, 4, 4], regular_sequencer_da, [4u8; 32]);
    let blob_5 = B::new(vec![5, 5, 5, 5, 5], preferred_sequencer_da, [5u8; 32]);
    let blob_6 = B::new(vec![6, 6, 6, 6, 6, 6], regular_sequencer_da, [6u8; 32]);
    let blob_7 = B::new(vec![7, 7, 7, 7, 7, 7, 7], regular_sequencer_da, [7u8; 32]);
    let blob_8 = B::new(
        vec![8, 8, 8, 8, 8, 8, 8, 8],
        regular_sequencer_da,
        [8u8; 32],
    );

    let mut slot_1 = vec![blob_1.clone(), blob_2.clone(), blob_3.clone()];
    let mut slot_2 = vec![blob_4.clone(), blob_5.clone(), blob_6.clone()];
    let mut slot_3 = vec![blob_7.clone(), blob_8.clone()];
    let mut slot_4 = vec![];

    // Slot 1: 3rd blob is from preferred sequencer, only it should be executed
    let mut execute_in_slot_1 = <BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
        &blob_storage,
        &mut slot_1,
        &mut working_set,
    )
    .unwrap();
    assert_eq!(1, execute_in_slot_1.len());
    blobs_are_equal(blob_3, execute_in_slot_1.remove(0), "slot 1");

    // Slot 2: 5th blob is from preferred sequencer + 2nd and 3rd that were deferred previously
    let mut execute_in_slot_2 = <BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
        &blob_storage,
        &mut slot_2,
        &mut working_set,
    )
    .unwrap();
    assert_eq!(3, execute_in_slot_2.len());
    blobs_are_equal(blob_5, execute_in_slot_2.remove(0), "slot 2");
    blobs_are_equal(blob_1, execute_in_slot_2.remove(0), "slot 2");
    blobs_are_equal(blob_2, execute_in_slot_2.remove(0), "slot 2");

    // Slot 3: no blobs from preferred sequencer, so deferred executed first and then current
    let mut execute_in_slot_3 = <BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
        &blob_storage,
        &mut slot_3,
        &mut working_set,
    )
    .unwrap();
    assert_eq!(2, execute_in_slot_3.len());
    blobs_are_equal(blob_4, execute_in_slot_3.remove(0), "slot 3");
    blobs_are_equal(blob_6, execute_in_slot_3.remove(0), "slot 3");

    let mut execute_in_slot_4 = <BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
        &blob_storage,
        &mut slot_4,
        &mut working_set,
    )
    .unwrap();

    assert_eq!(2, execute_in_slot_4.len());
    blobs_are_equal(blob_7, execute_in_slot_4.remove(0), "slot 4");
    blobs_are_equal(blob_8, execute_in_slot_4.remove(0), "slot 4");
}

#[test]
fn test_blobs_from_non_registered_sequencers_are_not_saved() {
    let tmpdir = tempfile::tempdir().unwrap();
    let mut working_set = WorkingSet::new(ProverStorage::with_path(tmpdir.path()).unwrap());

    let preferred_sequencer_da = MockAddress::from([10u8; 32]);
    let preferred_sequencer_rollup = generate_address(PREFERRED_SEQUENCER_KEY);
    let regular_sequencer_da = MockAddress::from([30u8; 32]);
    let regular_sequencer_rollup = generate_address(REGULAR_SEQUENCER_KEY);
    let some_sequencer = MockAddress::from([40u8; 32]);

    let bank_config = get_bank_config(preferred_sequencer_rollup, regular_sequencer_rollup);

    let token_address = sov_bank::get_genesis_token_address::<C>(
        &bank_config.tokens[0].token_name,
        bank_config.tokens[0].salt,
    );

    let sequencer_registry_config = SequencerConfig {
        seq_rollup_address: preferred_sequencer_rollup,
        seq_da_address: preferred_sequencer_da.as_ref().to_vec(),
        coins_to_lock: sov_bank::Coins {
            amount: LOCKED_AMOUNT,
            token_address,
        },
        is_preferred_sequencer: true,
    };

    let bank = sov_bank::Bank::<C>::default();
    let sequencer_registry = SequencerRegistry::<C>::default();
    let blob_storage = BlobStorage::<C>::default();

    bank.genesis(&bank_config, &mut working_set).unwrap();
    sequencer_registry
        .genesis(&sequencer_registry_config, &mut working_set)
        .unwrap();
    blob_storage.genesis(&(), &mut working_set).unwrap();

    let register_message = sov_sequencer_registry::CallMessage::Register {
        da_address: regular_sequencer_da.as_ref().to_vec(),
    };
    sequencer_registry
        .call(
            register_message,
            &C::new(regular_sequencer_rollup),
            &mut working_set,
        )
        .unwrap();

    let blob_1 = B::new(vec![1], regular_sequencer_da, [1u8; 32]);
    let blob_2 = B::new(vec![2, 2], some_sequencer, [2u8; 32]);
    let blob_3 = B::new(vec![3, 3, 3], preferred_sequencer_da, [3u8; 32]);

    let mut slot_1 = vec![blob_1.clone(), blob_2, blob_3.clone()];
    let mut slot_2 = vec![];

    let mut execute_in_slot_1 = <BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
        &blob_storage,
        &mut slot_1,
        &mut working_set,
    )
    .unwrap();
    assert_eq!(1, execute_in_slot_1.len());
    blobs_are_equal(blob_3, execute_in_slot_1.remove(0), "slot 1");

    let mut execute_in_slot_2 = <BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
        &blob_storage,
        &mut slot_2,
        &mut working_set,
    )
    .unwrap();
    assert_eq!(1, execute_in_slot_2.len());
    blobs_are_equal(blob_1, execute_in_slot_2.remove(0), "slot 2");
}

#[test]
fn test_blobs_no_deferred_without_preferred_sequencer() {
    let tmpdir = tempfile::tempdir().unwrap();
    let mut working_set = WorkingSet::new(ProverStorage::with_path(tmpdir.path()).unwrap());

    let preferred_sequencer_da = MockAddress::from([10u8; 32]);
    let preferred_sequencer_rollup = generate_address(PREFERRED_SEQUENCER_KEY);
    let regular_sequencer_da = MockAddress::from([30u8; 32]);
    let regular_sequencer_rollup = generate_address(REGULAR_SEQUENCER_KEY);

    let bank_config = get_bank_config(preferred_sequencer_rollup, regular_sequencer_rollup);

    let token_address = sov_bank::get_genesis_token_address::<C>(
        &bank_config.tokens[0].token_name,
        bank_config.tokens[0].salt,
    );

    let sequencer_registry_config = SequencerConfig {
        seq_rollup_address: preferred_sequencer_rollup,
        seq_da_address: preferred_sequencer_da.as_ref().to_vec(),
        coins_to_lock: sov_bank::Coins {
            amount: LOCKED_AMOUNT,
            token_address,
        },
        is_preferred_sequencer: false,
    };

    let bank = sov_bank::Bank::<C>::default();
    let sequencer_registry = SequencerRegistry::<C>::default();
    let blob_storage = BlobStorage::<C>::default();

    bank.genesis(&bank_config, &mut working_set).unwrap();
    sequencer_registry
        .genesis(&sequencer_registry_config, &mut working_set)
        .unwrap();
    blob_storage.genesis(&(), &mut working_set).unwrap();

    let register_message = sov_sequencer_registry::CallMessage::Register {
        da_address: regular_sequencer_da.as_ref().to_vec(),
    };
    sequencer_registry
        .call(
            register_message,
            &C::new(regular_sequencer_rollup),
            &mut working_set,
        )
        .unwrap();

    let blob_1 = B::new(vec![1], regular_sequencer_da, [1u8; 32]);
    let blob_2 = B::new(vec![2, 2], regular_sequencer_da, [2u8; 32]);
    let blob_3 = B::new(vec![3, 3, 3], preferred_sequencer_da, [3u8; 32]);

    let mut slot_1 = vec![blob_1.clone(), blob_2.clone(), blob_3.clone()];
    let mut slot_2: Vec<_> = vec![];

    let mut execute_in_slot_1 = <BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
        &blob_storage,
        &mut slot_1,
        &mut working_set,
    )
    .unwrap();
    assert_eq!(3, execute_in_slot_1.len());
    blobs_are_equal(blob_1, execute_in_slot_1.remove(0), "slot 1");
    blobs_are_equal(blob_2, execute_in_slot_1.remove(0), "slot 1");
    blobs_are_equal(blob_3, execute_in_slot_1.remove(0), "slot 1");

    let execute_in_slot_2: Vec<BlobRefOrOwned<'_, B>> =
        <BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
            &blob_storage,
            &mut slot_2,
            &mut working_set,
        )
        .unwrap();
    assert!(execute_in_slot_2.is_empty());
}

#[test]
fn deferred_blobs_are_first_after_preferred_sequencer_exit() {
    let tmpdir = tempfile::tempdir().unwrap();
    let mut working_set = WorkingSet::new(ProverStorage::with_path(tmpdir.path()).unwrap());

    let preferred_sequencer_da = MockAddress::from([10u8; 32]);
    let preferred_sequencer_rollup = generate_address(PREFERRED_SEQUENCER_KEY);
    let regular_sequencer_da = MockAddress::from([30u8; 32]);
    let regular_sequencer_rollup = generate_address(REGULAR_SEQUENCER_KEY);

    let bank_config = get_bank_config(preferred_sequencer_rollup, regular_sequencer_rollup);

    let token_address = sov_bank::get_genesis_token_address::<C>(
        &bank_config.tokens[0].token_name,
        bank_config.tokens[0].salt,
    );

    let sequencer_registry_config = SequencerConfig {
        seq_rollup_address: preferred_sequencer_rollup,
        seq_da_address: preferred_sequencer_da.as_ref().to_vec(),
        coins_to_lock: sov_bank::Coins {
            amount: LOCKED_AMOUNT,
            token_address,
        },
        is_preferred_sequencer: true,
    };

    let bank = sov_bank::Bank::<C>::default();
    let sequencer_registry = SequencerRegistry::<C>::default();
    let blob_storage = BlobStorage::<C>::default();

    bank.genesis(&bank_config, &mut working_set).unwrap();
    sequencer_registry
        .genesis(&sequencer_registry_config, &mut working_set)
        .unwrap();
    blob_storage.genesis(&(), &mut working_set).unwrap();

    let register_message = sov_sequencer_registry::CallMessage::Register {
        da_address: regular_sequencer_da.as_ref().to_vec(),
    };
    sequencer_registry
        .call(
            register_message,
            &C::new(regular_sequencer_rollup),
            &mut working_set,
        )
        .unwrap();

    let blob_1 = B::new(vec![1], regular_sequencer_da, [1u8; 32]);
    let blob_2 = B::new(vec![2, 2], regular_sequencer_da, [2u8; 32]);
    let blob_3 = B::new(vec![3, 3, 3], preferred_sequencer_da, [3u8; 32]);
    let blob_4 = B::new(vec![4, 4, 4, 4], regular_sequencer_da, [4u8; 32]);
    let blob_5 = B::new(vec![5, 5, 5, 5, 5], regular_sequencer_da, [5u8; 32]);

    let mut slot_1 = vec![blob_1.clone(), blob_2.clone(), blob_3.clone()];
    let mut slot_2 = vec![blob_4.clone(), blob_5.clone()];
    let mut slot_3: Vec<_> = vec![];

    let mut execute_in_slot_1 = <BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
        &blob_storage,
        &mut slot_1,
        &mut working_set,
    )
    .unwrap();

    assert_eq!(1, execute_in_slot_1.len());
    blobs_are_equal(blob_3, execute_in_slot_1.remove(0), "slot 1");

    let exit_message = sov_sequencer_registry::CallMessage::Exit {
        da_address: preferred_sequencer_da.as_ref().to_vec(),
    };

    sequencer_registry
        .call(
            exit_message,
            &C::new(preferred_sequencer_rollup),
            &mut working_set,
        )
        .unwrap();

    assert!(sequencer_registry
        .get_preferred_sequencer(&mut working_set)
        .is_none());

    let mut execute_in_slot_2 = <BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
        &blob_storage,
        &mut slot_2,
        &mut working_set,
    )
    .unwrap();
    assert_eq!(4, execute_in_slot_2.len());
    blobs_are_equal(blob_1, execute_in_slot_2.remove(0), "slot 2");
    blobs_are_equal(blob_2, execute_in_slot_2.remove(0), "slot 2");
    blobs_are_equal(blob_4, execute_in_slot_2.remove(0), "slot 2");
    blobs_are_equal(blob_5, execute_in_slot_2.remove(0), "slot 2");

    let execute_in_slot_3: Vec<BlobRefOrOwned<'_, B>> =
        <BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
            &blob_storage,
            &mut slot_3,
            &mut working_set,
        )
        .unwrap();
    assert!(execute_in_slot_3.is_empty());
}

/// Check hashes and data of two blobs.
fn blobs_are_equal<B: BlobReaderTrait>(
    mut expected: B,
    mut actual: BlobRefOrOwned<B>,
    slot_hint: &str,
) {
    let actual_inner = actual.as_mut_ref();
    assert_eq!(
        expected.hash(),
        actual_inner.hash(),
        "incorrect hashes in {}",
        slot_hint
    );

    let mut read_actual = vec![];
    actual_inner
        .data_mut()
        .read_to_end(&mut read_actual)
        .unwrap();

    let mut read_expected = vec![];
    expected.data_mut().read_to_end(&mut read_expected).unwrap();
    assert_eq!(
        read_expected, read_actual,
        "incorrect data read in {}",
        slot_hint
    );
}
