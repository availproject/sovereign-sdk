use std::hash::Hasher;

use jsonrpsee::core::RpcResult;
use sov_modules_api::default_context::ZkDefaultContext;
use sov_modules_api::macros::rpc_gen;
use sov_modules_api::{Context, ModuleInfo};
use sov_state::{WorkingSet, ZkStorage};

#[derive(ModuleInfo)]
pub struct TestStruct<C: ::sov_modules_api::Context, D>
where
    D: std::hash::Hash
        + std::clone::Clone
        + borsh::BorshSerialize
        + borsh::BorshDeserialize
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static,
{
    #[address]
    pub(crate) address: C::Address,
    #[state]
    pub(crate) data: ::sov_state::StateValue<D>,
}

#[rpc_gen(client, server, namespace = "test")]
impl<C: sov_modules_api::Context, D> TestStruct<C, D>
where
    D: std::hash::Hash
        + std::clone::Clone
        + borsh::BorshSerialize
        + borsh::BorshDeserialize
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static,
{
    #[rpc_method(name = "firstMethod")]
    pub fn first_method(&self, _working_set: &mut WorkingSet<C::Storage>) -> RpcResult<u32> {
        Ok(11)
    }

    #[rpc_method(name = "secondMethod")]
    pub fn second_method(
        &self,
        result: D,
        _working_set: &mut WorkingSet<C::Storage>,
    ) -> RpcResult<(D, u64)> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let value = result.clone();
        value.hash(&mut hasher);
        let hashed_value = hasher.finish();

        Ok((value, hashed_value))
    }
}

pub struct TestRuntime<C: Context> {
    test_struct: TestStruct<C, u32>,
}

// This is generated by a macro annotating the state transition runner,
// but we do not have that in scope here so generating the struct manually.
struct RpcStorage<C: Context> {
    pub storage: C::Storage,
}

impl TestStructRpcImpl<ZkDefaultContext, u32> for RpcStorage<ZkDefaultContext> {
    fn get_working_set(
        &self,
    ) -> ::sov_state::WorkingSet<<ZkDefaultContext as ::sov_modules_api::Spec>::Storage> {
        ::sov_state::WorkingSet::new(self.storage.clone())
    }
}

fn main() {
    let storage = ZkStorage::new([1u8; 32]);
    let r: RpcStorage<ZkDefaultContext> = RpcStorage {
        storage: storage.clone(),
    };
    {
        let result =
            <RpcStorage<ZkDefaultContext> as TestStructRpcServer<ZkDefaultContext, u32>>::first_method(
                &r,
            )
            .unwrap();
        assert_eq!(result, 11);
    }

    {
        let result =
            <RpcStorage<ZkDefaultContext> as TestStructRpcServer<ZkDefaultContext, u32>>::second_method(
                &r, 22,
            )
            .unwrap();
        assert_eq!(result, (22, 15733059416522709050));
    }

    println!("All tests passed!");
}
