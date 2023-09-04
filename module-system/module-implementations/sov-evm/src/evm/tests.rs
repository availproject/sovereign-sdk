use std::convert::Infallible;

use reth_primitives::TransactionKind;
use revm::db::CacheDB;
use revm::primitives::{CfgEnv, ExecutionResult, Output, KECCAK_EMPTY, U256};
use revm::{Database, DatabaseCommit};
use sov_state::{ProverStorage, WorkingSet};

use super::db::EvmDb;
use super::db_init::InitEvmDb;
use super::executor;
use crate::evm::transaction::BlockEnv;
use crate::evm::{contract_address, AccountInfo};
use crate::smart_contracts::SimpleStorageContract;
use crate::tests::dev_signer::DevSigner;
use crate::Evm;
type C = sov_modules_api::default_context::DefaultContext;

pub(crate) fn output(result: ExecutionResult) -> bytes::Bytes {
    match result {
        ExecutionResult::Success { output, .. } => match output {
            Output::Call(out) => out,
            Output::Create(out, _) => out,
        },
        _ => panic!("Expected successful ExecutionResult"),
    }
}

#[test]
fn simple_contract_execution_sov_state() {
    let tmpdir = tempfile::tempdir().unwrap();
    let mut working_set: WorkingSet<<C as sov_modules_api::Spec>::Storage> =
        WorkingSet::new(ProverStorage::with_path(tmpdir.path()).unwrap());

    let evm = Evm::<C>::default();
    let evm_db: EvmDb<'_, C> = evm.get_db(&mut working_set);

    simple_contract_execution(evm_db);
}

#[test]
fn simple_contract_execution_in_memory_state() {
    let db = CacheDB::default();
    simple_contract_execution(db);
}

fn simple_contract_execution<DB: Database<Error = Infallible> + DatabaseCommit + InitEvmDb>(
    mut evm_db: DB,
) {
    let dev_signer = DevSigner::new_random();
    let caller = dev_signer.address;
    evm_db.insert_account_info(
        caller,
        AccountInfo {
            balance: U256::from(1000000000).to_le_bytes(),
            code_hash: KECCAK_EMPTY.to_fixed_bytes(),
            code: vec![],
            nonce: 1,
        },
    );

    let contract = SimpleStorageContract::default();

    let contract_address = {
        let tx = dev_signer
            .sign_default_transaction(TransactionKind::Create, contract.byte_code().to_vec(), 1)
            .unwrap();

        let tx = &tx.try_into().unwrap();
        let result =
            executor::execute_tx(&mut evm_db, BlockEnv::default(), tx, CfgEnv::default()).unwrap();
        contract_address(result).expect("Expected successful contract creation")
    };

    let set_arg = 21989;

    {
        let call_data = contract.set_call_data(set_arg);

        let tx = dev_signer
            .sign_default_transaction(
                TransactionKind::Call(contract_address.as_fixed_bytes().into()),
                hex::decode(hex::encode(&call_data)).unwrap(),
                2,
            )
            .unwrap();

        let tx = &tx.try_into().unwrap();
        executor::execute_tx(&mut evm_db, BlockEnv::default(), tx, CfgEnv::default()).unwrap();
    }

    let get_res = {
        let call_data = contract.get_call_data();

        let tx = dev_signer
            .sign_default_transaction(
                TransactionKind::Call(contract_address.as_fixed_bytes().into()),
                hex::decode(hex::encode(&call_data)).unwrap(),
                3,
            )
            .unwrap();

        let tx = &tx.try_into().unwrap();
        let result =
            executor::execute_tx(&mut evm_db, BlockEnv::default(), tx, CfgEnv::default()).unwrap();

        let out = output(result);
        ethereum_types::U256::from(out.as_ref())
    };

    assert_eq!(set_arg, get_res.as_u32())
}
