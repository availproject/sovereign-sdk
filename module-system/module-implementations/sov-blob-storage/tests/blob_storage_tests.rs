use sov_blob_storage::BlobStorage;
use sov_modules_api::default_context::DefaultContext;
use sov_modules_api::Genesis;
use sov_rollup_interface::mocks::{MockAddress, MockBlob};
use sov_state::{ProverStorage, WorkingSet};

type C = DefaultContext;
type B = MockBlob;

#[test]
fn empty_test() {
    let tmpdir = tempfile::tempdir().unwrap();
    let mut working_set = WorkingSet::new(ProverStorage::with_path(tmpdir.path()).unwrap());
    let blob_storage = BlobStorage::<C>::default();

    blob_storage.genesis(&(), &mut working_set).unwrap();

    let blobs: Vec<B> = blob_storage.take_blobs_for_block_number(1, &mut working_set);

    assert!(blobs.is_empty());
}

#[test]
fn store_and_retrieve_standard() {
    let tmpdir = tempfile::tempdir().unwrap();
    let mut working_set = WorkingSet::new(ProverStorage::with_path(tmpdir.path()).unwrap());
    let blob_storage = BlobStorage::<C>::default();

    blob_storage.genesis(&(), &mut working_set).unwrap();

    assert!(blob_storage
        .take_blobs_for_block_number::<B>(1, &mut working_set)
        .is_empty());
    assert!(blob_storage
        .take_blobs_for_block_number::<B>(2, &mut working_set)
        .is_empty());
    assert!(blob_storage
        .take_blobs_for_block_number::<B>(3, &mut working_set)
        .is_empty());
    assert!(blob_storage
        .take_blobs_for_block_number::<B>(4, &mut working_set)
        .is_empty());

    let sender = MockAddress::from([1u8; 32]);
    let dummy_hash = [2u8; 32];

    let blob_1 = B::new(vec![1, 2, 3], sender, dummy_hash);
    let blob_2 = B::new(vec![3, 4, 5], sender, dummy_hash);
    let blob_3 = B::new(vec![6, 7, 8], sender, dummy_hash);
    let blob_4 = B::new(vec![9, 9, 9], sender, dummy_hash);
    let blob_5 = B::new(vec![0, 1, 0], sender, dummy_hash);

    let slot_2_blobs = vec![blob_1, blob_2, blob_3];
    let slot_2_blob_refs: Vec<&MockBlob> = slot_2_blobs.iter().collect();
    let slot_3_blobs = vec![blob_4];
    let slot_3_blob_refs: Vec<&MockBlob> = slot_3_blobs.iter().collect();
    let slot_4_blobs = vec![blob_5];
    let slot_4_blob_refs: Vec<&MockBlob> = slot_4_blobs.iter().collect();

    blob_storage
        .store_blobs(2, &slot_2_blob_refs, &mut working_set)
        .unwrap();
    blob_storage
        .store_blobs(3, &slot_3_blob_refs, &mut working_set)
        .unwrap();
    blob_storage
        .store_blobs(4, &slot_4_blob_refs, &mut working_set)
        .unwrap();

    assert_eq!(
        slot_2_blobs,
        blob_storage.take_blobs_for_block_number(2, &mut working_set)
    );
    assert!(blob_storage
        .take_blobs_for_block_number::<B>(2, &mut working_set)
        .is_empty());

    assert_eq!(
        slot_3_blobs,
        blob_storage.take_blobs_for_block_number(3, &mut working_set)
    );
    assert!(blob_storage
        .take_blobs_for_block_number::<B>(3, &mut working_set)
        .is_empty());

    assert_eq!(
        slot_4_blobs,
        blob_storage.take_blobs_for_block_number(4, &mut working_set)
    );
    assert!(blob_storage
        .take_blobs_for_block_number::<B>(4, &mut working_set)
        .is_empty());
}
