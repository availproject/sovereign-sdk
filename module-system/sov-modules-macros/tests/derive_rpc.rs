use sov_modules_api::default_context::ZkDefaultContext;
use sov_modules_api::macros::rpc_gen;
use sov_modules_api::{Context, ModuleInfo};
use sov_state::{WorkingSet, ZkStorage};

#[derive(ModuleInfo)]
pub struct TestStruct<C: ::sov_modules_api::Context> {
    #[address]
    pub(crate) address: C::Address,
}

#[rpc_gen(client, server, namespace = "test")]
impl<C: sov_modules_api::Context> TestStruct<C> {
    #[rpc_method(name = "firstMethod")]
    pub fn first_method(&self, _working_set: &mut WorkingSet<C::Storage>) -> u32 {
        11
    }

    #[rpc_method(name = "secondMethod")]
    pub fn second_method(&self, result: u32, _working_set: &mut WorkingSet<C::Storage>) -> u32 {
        result
    }

    #[rpc_method(name = "thirdMethod")]
    pub fn third_method(&self, result: u32) -> u32 {
        result
    }

    #[rpc_method(name = "fourthMethod")]
    pub fn fourth_method(&self, _working_set: &mut WorkingSet<C::Storage>, result: u32) -> u32 {
        result
    }
}

pub struct TestRuntime<C: Context> {
    test_struct: TestStruct<C>,
}

// This is generated by a macro annotating the state transition runner,
// but we do not have that in scope here so generating the struct manually.
struct RpcStorage<C: Context> {
    pub storage: C::Storage,
}

impl TestStructRpcImpl<ZkDefaultContext> for RpcStorage<ZkDefaultContext> {
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
            <RpcStorage<ZkDefaultContext> as TestStructRpcServer<ZkDefaultContext>>::first_method(
                &r,
            );
        assert_eq!(result.unwrap(), 11);
    }

    {
        let result =
            <RpcStorage<ZkDefaultContext> as TestStructRpcServer<ZkDefaultContext>>::second_method(
                &r, 22,
            );
        assert_eq!(result.unwrap(), 22);
    }

    {
        let result =
            <RpcStorage<ZkDefaultContext> as TestStructRpcServer<ZkDefaultContext>>::third_method(
                &r, 33,
            );
        assert_eq!(result.unwrap(), 33);
    }

    {
        let result =
            <RpcStorage<ZkDefaultContext> as TestStructRpcServer<ZkDefaultContext>>::fourth_method(
                &r, 44,
            );
        assert_eq!(result.unwrap(), 44);
    }

    {
        let result =
            <RpcStorage<ZkDefaultContext> as TestStructRpcServer<ZkDefaultContext>>::health(&r);
        assert_eq!(result.unwrap(), ());
    }

    println!("All tests passed!")
}
