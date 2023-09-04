use module_template::{CallMessage, ExampleModule, ExampleModuleConfig, Response};
#[cfg(feature = "native")]
use sov_modules_api::default_context::DefaultContext;
use sov_modules_api::default_context::ZkDefaultContext;
use sov_modules_api::{Address, Context, Module};
use sov_rollup_interface::stf::Event;
use sov_state::{DefaultStorageSpec, ProverStorage, WorkingSet, ZkStorage};

#[test]
fn test_value_setter() {
    let tmpdir = tempfile::tempdir().unwrap();

    #[cfg(feature = "native")]
    let mut working_set =
        WorkingSet::new(ProverStorage::<DefaultStorageSpec>::with_path(tmpdir.path()).unwrap());

    let admin = Address::from([1; 32]);
    // Test Native-Context
    #[cfg(feature = "native")]
    {
        let config = ExampleModuleConfig {};
        let context = DefaultContext::new(admin);
        test_value_setter_helper(context, &config, &mut working_set);
    }

    let (_, witness) = working_set.checkpoint().freeze();

    // Test Zk-Context
    {
        let config = ExampleModuleConfig {};
        let zk_context = ZkDefaultContext::new(admin);
        let mut zk_working_set = WorkingSet::with_witness(ZkStorage::new([0u8; 32]), witness);
        test_value_setter_helper(zk_context, &config, &mut zk_working_set);
    }
}

fn test_value_setter_helper<C: Context>(
    context: C,
    config: &ExampleModuleConfig,
    working_set: &mut WorkingSet<C::Storage>,
) {
    let module = ExampleModule::<C>::default();
    module.genesis(config, working_set).unwrap();

    let new_value = 99;
    let call_msg = CallMessage::SetValue(new_value);

    // Test events
    {
        module.call(call_msg, &context, working_set).unwrap();
        let event = &working_set.events()[0];
        assert_eq!(event, &Event::new("set", "value_set: 99"));
    }

    // Test query
    #[cfg(feature = "native")]
    {
        let query_response = module.query_value(working_set);
        assert_eq!(
            Response {
                value: Some(new_value)
            },
            query_response
        )
    }
}
