use jsonrpsee::RpcModule;
use serde::{de::DeserializeOwned, Serialize};
use sov_db::ledger_db::LedgerDB;
use sov_rollup_interface::rpc::{
    BatchIdentifier, EventIdentifier, LedgerRpcProvider, SlotIdentifier, TxIdentifier,
};

use self::query_args::{extract_query_args, QueryArgs};

/// Registers the following RPC methods
/// - `ledger_head`
///    Example Query: `curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"ledger_head","params":[],"id":1}' http://127.0.0.1:12345`
/// - ledger_getSlots
///    Example Query: `curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"ledger_getSlots","params":[[1, 2], "Compact"],"id":1}' http://127.0.0.1:12345`
/// - ledger_getBatches
///    Example Query: `curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"ledger_getBatches","params":[[1, 2], "Standard"],"id":1}' http://127.0.0.1:12345`
/// - ledger_getTransactions
///    Example Query: `curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"ledger_getBatches","params":[[1, 2], "Full"],"id":1}' http://127.0.0.1:12345`
/// - ledger_getEvents
///    Example Query: `curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"ledger_getBatches","params":[1, 2],"id":1}' http://127.0.0.1:12345`
fn register_ledger_rpc_methods<B: Serialize + DeserializeOwned, T: Serialize + DeserializeOwned>(
    rpc: &mut RpcModule<LedgerDB>,
) -> Result<(), jsonrpsee::core::Error> {
    rpc.register_method("ledger_getHead", move |_, db| {
        db.get_head::<B, T>().map_err(|e| e.into())
    })?;

    rpc.register_method("ledger_getSlots", move |params, db| {
        let args: QueryArgs<SlotIdentifier> = extract_query_args(params)?;
        db.get_slots::<B, T>(&args.0, args.1).map_err(|e| e.into())
    })?;

    rpc.register_method("ledger_getBatches", move |params, db| {
        let args: QueryArgs<BatchIdentifier> = extract_query_args(params)?;
        db.get_batches::<B, T>(&args.0, args.1)
            .map_err(|e| e.into())
    })?;

    rpc.register_method("ledger_getTransactions", move |params, db| {
        let args: QueryArgs<TxIdentifier> = extract_query_args(params)?;
        db.get_transactions::<T>(&args.0, args.1)
            .map_err(|e| e.into())
    })?;

    rpc.register_method("ledger_getEvents", move |params, db| {
        let ids: Vec<EventIdentifier> = params.parse()?;
        db.get_events(&ids).map_err(|e| e.into())
    })?;

    Ok(())
}

pub fn get_ledger_rpc<B: Serialize + DeserializeOwned, T: Serialize + DeserializeOwned>(
    ledger_db: LedgerDB,
) -> RpcModule<LedgerDB> {
    let mut rpc = RpcModule::new(ledger_db);
    register_ledger_rpc_methods::<B, T>(&mut rpc).expect("Failed to register ledger RPC methods");
    rpc
}

mod query_args {
    use serde::de::DeserializeOwned;
    use sov_rollup_interface::rpc::QueryMode;

    #[derive(serde::Deserialize)]
    pub struct QueryArgs<I>(pub Vec<I>, #[serde(default)] pub QueryMode);

    /// Extract the args from an RPC query, being liberal in what is accepted.
    /// To query for a list of items, users can either pass a list of ids, or tuple containing a list of ids and a query mode
    pub fn extract_query_args<I: DeserializeOwned>(
        params: jsonrpsee::types::Params,
    ) -> Result<QueryArgs<I>, jsonrpsee::core::Error> {
        if let Ok(args) = params.parse() {
            return Ok(args);
        }
        let ids: Vec<I> = params.parse()?;
        Ok(QueryArgs(ids, Default::default()))
    }
}
