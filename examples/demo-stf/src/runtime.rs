#[cfg(feature = "native")]
use sov_accounts::query::{AccountsRpcImpl, AccountsRpcServer};
#[cfg(feature = "native")]
use sov_bank::query::{BankRpcImpl, BankRpcServer};
#[cfg(feature = "native")]
use sov_blob_storage::{BlobStorageRpcImpl, BlobStorageRpcServer};
#[cfg(feature = "native")]
use sov_chain_state::{ChainStateRpcImpl, ChainStateRpcServer};
#[cfg(feature = "native")]
#[cfg(feature = "experimental")]
use sov_evm::query::{EvmRpcImpl, EvmRpcServer};
use sov_modules_api::capabilities::{BlobRefOrOwned, BlobSelector};
#[cfg(feature = "native")]
pub use sov_modules_api::default_context::DefaultContext;
use sov_modules_api::hooks::SlotHooks;
use sov_modules_api::macros::DefaultRuntime;
#[cfg(feature = "native")]
use sov_modules_api::macros::{expose_rpc, CliWallet};
use sov_modules_api::{Context, DispatchCall, Genesis, MessageCodec, Spec};
use sov_rollup_interface::da::DaSpec;
#[cfg(feature = "native")]
use sov_sequencer_registry::{SequencerRegistryRpcImpl, SequencerRegistryRpcServer};
#[cfg(feature = "native")]
use sov_value_setter::query::{ValueSetterRpcImpl, ValueSetterRpcServer};

#[cfg(feature = "native")]
pub mod query {
    pub use sov_accounts::query as accounts;
    pub use sov_bank::query as bank;
    pub use sov_chain_state::query as chain_state;
    pub use sov_sequencer_registry::query as sequencer_registry;
    pub use sov_value_setter::query as value_setter;
}

/// The Rollup entrypoint.
///
/// On a high level, the rollup node receives serialized call messages from the DA layer and executes them as atomic transactions.
/// Upon reception, the message has to be deserialized and forwarded to an appropriate module.
///
/// The module-specific logic is implemented by module creators, but all the glue code responsible for message
/// deserialization/forwarding is handled by a rollup `runtime`.
///
/// In order to define the runtime we need to specify all the modules supported by our rollup (see the `Runtime` struct bellow)
///
/// The `Runtime` together with associated interfaces (`Genesis`, `DispatchCall`, `MessageCodec`)
/// and derive macros defines:
/// - how the rollup modules are wired up together.
/// - how the state of the rollup is initialized.
/// - how messages are dispatched to appropriate modules.
///
/// Runtime lifecycle:
///
/// 1. Initialization:
///     When a rollup is deployed for the first time, it needs to set its genesis state.
///     The `#[derive(Genesis)` macro will generate `Runtime::genesis(config)` method which returns
///     `Storage` with the initialized state.
///
/// 2. Calls:      
///     The `Module` interface defines a `call` method which accepts a module-defined type and triggers the specific `module logic.`
///     In general, the point of a call is to change the module state, but if the call throws an error,
///     no state is updated (the transaction is reverted).
///
/// `#[derive(MessageCodec)` adds deserialization capabilities to the `Runtime` (implements `decode_call` method).
/// `Runtime::decode_call` accepts serialized call message and returns a type that implements the `DispatchCall` trait.
///  The `DispatchCall` implementation (derived by a macro) forwards the message to the appropriate module and executes its `call` method.
///
/// Similar mechanism works for queries with the difference that queries are submitted by users directly to the rollup node
/// instead of going through the DA layer.

#[cfg(not(feature = "experimental"))]
#[cfg_attr(feature = "native", derive(CliWallet), expose_rpc)]
#[derive(Genesis, DispatchCall, MessageCodec, DefaultRuntime)]
#[serialization(borsh::BorshDeserialize, borsh::BorshSerialize)]
#[cfg_attr(
    feature = "native",
    serialization(serde::Serialize, serde::Deserialize)
)]
pub struct Runtime<C: Context, Da: DaSpec> {
    pub bank: sov_bank::Bank<C>,
    pub sequencer_registry: sov_sequencer_registry::SequencerRegistry<C>,
    #[cfg_attr(feature = "native", cli_skip)]
    pub blob_storage: sov_blob_storage::BlobStorage<C>,
    #[cfg_attr(feature = "native", cli_skip)]
    pub chain_state: sov_chain_state::ChainState<C, Da>,
    pub value_setter: sov_value_setter::ValueSetter<C>,
    pub accounts: sov_accounts::Accounts<C>,
}

#[cfg(feature = "experimental")]
#[cfg_attr(feature = "native", derive(CliWallet), expose_rpc)]
#[derive(Genesis, DispatchCall, MessageCodec, DefaultRuntime)]
#[serialization(borsh::BorshDeserialize, borsh::BorshSerialize)]
#[cfg_attr(
    feature = "native",
    serialization(serde::Serialize, serde::Deserialize)
)]
pub struct Runtime<C: Context, Da: DaSpec> {
    pub bank: sov_bank::Bank<C>,
    pub sequencer_registry: sov_sequencer_registry::SequencerRegistry<C>,
    #[cfg_attr(feature = "native", cli_skip)]
    pub blob_storage: sov_blob_storage::BlobStorage<C>,
    #[cfg_attr(feature = "native", cli_skip)]
    pub chain_state: sov_chain_state::ChainState<C, Da>,
    pub value_setter: sov_value_setter::ValueSetter<C>,
    pub accounts: sov_accounts::Accounts<C>,
    #[cfg_attr(feature = "native", cli_skip)]
    pub evm: sov_evm::Evm<C>,
}

impl<C: Context, Da: DaSpec> SlotHooks<Da> for Runtime<C, Da> {
    type Context = C;

    fn begin_slot_hook(
        &self,
        _slot_data: &impl sov_rollup_interface::services::da::SlotData,
        _working_set: &mut sov_state::WorkingSet<<Self::Context as Spec>::Storage>,
    ) {
    }

    fn end_slot_hook(
        &self,
        #[allow(unused_variables)] root_hash: [u8; 32],
        #[allow(unused_variables)] working_set: &mut sov_state::WorkingSet<
            <Self::Context as Spec>::Storage,
        >,
    ) {
        #[cfg(feature = "experimental")]
        self.evm.end_slot_hook(root_hash, working_set);
    }
}

impl<C, Da> sov_modules_stf_template::Runtime<C, Da> for Runtime<C, Da>
where
    C: Context,
    Da: DaSpec,
{
}

impl<C: Context, Da: DaSpec> BlobSelector<Da> for Runtime<C, Da> {
    type Context = C;

    fn get_blobs_for_this_slot<'a, I>(
        &self,
        current_blobs: I,
        working_set: &mut sov_state::WorkingSet<<Self::Context as Spec>::Storage>,
    ) -> anyhow::Result<Vec<BlobRefOrOwned<'a, Da::BlobTransaction>>>
    where
        I: IntoIterator<Item = &'a mut Da::BlobTransaction>,
    {
        <sov_blob_storage::BlobStorage<C> as BlobSelector<Da>>::get_blobs_for_this_slot(
            &self.blob_storage,
            current_blobs,
            working_set,
        )
    }
}
