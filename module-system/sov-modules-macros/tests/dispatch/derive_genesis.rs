mod modules;

use modules::{first_test_module, second_test_module};
use sov_modules_api::default_context::ZkDefaultContext;
use sov_modules_api::Context;
use sov_modules_macros::{DefaultRuntime, DispatchCall, Genesis, MessageCodec};
use sov_state::ZkStorage;

// Debugging hint: To expand the macro in tests run: `cargo expand --test tests`
#[derive(Genesis, DispatchCall, MessageCodec, DefaultRuntime)]
#[serialization(borsh::BorshDeserialize, borsh::BorshSerialize)]
struct Runtime<C>
where
    C: Context,
{
    pub first: first_test_module::FirstTestStruct<C>,
    pub second: second_test_module::SecondTestStruct<C>,
}

fn main() {
    use sov_modules_api::Genesis;

    type C = ZkDefaultContext;
    let storage = ZkStorage::new([1u8; 32]);
    let mut working_set = &mut sov_state::WorkingSet::new(storage);
    let runtime = &mut Runtime::<C>::default();
    let config = GenesisConfig::new((), ());
    runtime.genesis(&config, working_set).unwrap();

    {
        let response = runtime.first.get_state_value(&mut working_set);
        assert_eq!(response, 1);
    }

    {
        let response = runtime.second.get_state_value(&mut working_set);
        assert_eq!(response, 2);
    }
}
