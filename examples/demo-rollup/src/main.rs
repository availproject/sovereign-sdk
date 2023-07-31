use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use const_rollup_config::{ROLLUP_NAMESPACE_RAW, SEQUENCER_DA_ADDRESS};
use demo_stf::app::{
    DefaultContext, DefaultPrivateKey, DemoBatchReceipt, DemoTxReceipt, NativeAppRunner,
};
use demo_stf::genesis_config::create_demo_genesis_config;
use demo_stf::runner_config::from_toml_path;
use demo_stf::runtime::{get_rpc_methods, GenesisConfig};
use jsonrpsee::core::server::rpc_module::Methods;
use jupiter::da_service::CelestiaService;
#[cfg(feature = "experimental")]
use jupiter::da_service::DaServiceConfig;
use jupiter::types::NamespaceId;
use jupiter::verifier::{CelestiaVerifier, ChainValidityCondition, RollupParams};
use jupiter::BlobWithSender;
use risc0_adapter::host::Risc0Verifier;
use sov_db::ledger_db::{LedgerDB, SlotCommit};
#[cfg(feature = "experimental")]
use sov_ethereum::get_ethereum_rpc;
use sov_modules_api::RpcRunner;
use sov_rollup_interface::crypto::NoOpHasher;
use sov_rollup_interface::da::{BlockHeaderTrait, DaVerifier};
use sov_rollup_interface::services::da::{DaService, SlotData};
use sov_rollup_interface::services::stf_runner::StateTransitionRunner;
use sov_rollup_interface::stf::StateTransitionFunction;
use sov_rollup_interface::zk::ValidityConditionChecker;
// RPC related imports
use sov_sequencer::get_sequencer_rpc;
use sov_state::Storage;
use tracing::{debug, info, Level};

use crate::config::RollupConfig;

mod config;
mod ledger_rpc;

#[cfg(test)]
mod test_rpc;

#[cfg(feature = "experimental")]
const TX_SIGNER_PRIV_KEY_PATH: &str = "../test-data/keys/tx_signer_private_key.json";

// The rollup stores its data in the namespace b"sov-test" on Celestia
// You can change this constant to point your rollup at a different namespace
const ROLLUP_NAMESPACE: NamespaceId = NamespaceId(ROLLUP_NAMESPACE_RAW);

pub fn initialize_ledger(path: impl AsRef<std::path::Path>) -> LedgerDB {
    LedgerDB::with_path(path).expect("Ledger DB failed to open")
}

async fn start_rpc_server(methods: impl Into<Methods>, address: SocketAddr) {
    let server = jsonrpsee::server::ServerBuilder::default()
        .build([address].as_ref())
        .await
        .unwrap();

    info!("Starting RPC server at {} ", server.local_addr().unwrap());
    let _server_handle = server.start(methods).unwrap();
    futures::future::pending::<()>().await;
}

// TODO: Remove this when sov-cli is in its own crate.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct HexKey {
    hex_priv_key: String,
    address: String,
}

/// Configure our rollup with a centralized sequencer using the SEQUENCER_DA_ADDRESS
/// address constant. Since the centralize sequencer's address is consensus critical,
/// it has to be hardcoded as a constant, rather than read from the config at runtime.
///
/// If you want to customize the rollup to accept transactions from your own celestia
/// address, simply change the value of the SEQUENCER_DA_ADDRESS to your own address.
/// For example:
/// ```rust,no_run
/// const SEQUENCER_DA_ADDRESS: [u8;47] = *b"celestia1qp09ysygcx6npted5yc0au6k9lner05yvs9208"
/// ```
pub fn get_genesis_config() -> GenesisConfig<DefaultContext> {
    let hex_key: HexKey = serde_json::from_slice(include_bytes!(
        "../../test-data/keys/token_deployer_private_key.json"
    ))
    .expect("Broken key data file");
    let sequencer_private_key = DefaultPrivateKey::from_hex(&hex_key.hex_priv_key).unwrap();
    assert_eq!(
        sequencer_private_key.default_address().to_string(),
        hex_key.address,
        "Inconsistent key data",
    );
    create_demo_genesis_config(
        100000000,
        sequencer_private_key.default_address(),
        SEQUENCER_DA_ADDRESS.to_vec(),
        &sequencer_private_key,
        &sequencer_private_key,
    )
}

pub struct CelestiaChainChecker {
    current_block_hash: [u8; 32],
}

impl ValidityConditionChecker<ChainValidityCondition> for CelestiaChainChecker {
    type Error = anyhow::Error;

    fn check(&mut self, condition: &ChainValidityCondition) -> Result<(), anyhow::Error> {
        anyhow::ensure!(
            condition.block_hash == self.current_block_hash,
            "Invalid block hash"
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let rollup_config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "rollup_config.toml".to_string());

    debug!("Starting demo rollup with config {}", rollup_config_path);
    let rollup_config: RollupConfig =
        from_toml_path(&rollup_config_path).context("Failed to read rollup configuration")?;

    let rpc_config = rollup_config.rpc_config;
    let address = SocketAddr::new(rpc_config.bind_host.parse()?, rpc_config.bind_port);

    // Initializing logging
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_err| eprintln!("Unable to set global default subscriber"))
        .expect("Cannot fail to set subscriber");

    // Initialize the ledger database, which stores blocks, transactions, events, etc.
    let ledger_db = initialize_ledger(&rollup_config.runner.storage.path);

    // Initialize the Celestia service using the DaService interface
    let da_service = Arc::new(
        CelestiaService::new(
            rollup_config.da.clone(),
            RollupParams {
                namespace: ROLLUP_NAMESPACE,
            },
        )
        .await,
    );

    // Our state transition function implements the StateTransitionRunner interface,
    // so we use that to initialize the STF
    let mut demo_runner =
        NativeAppRunner::<Risc0Verifier, BlobWithSender>::new(rollup_config.runner.clone());

    // Our state transition also implements the RpcRunner interface,
    // so we use that to initialize the RPC server.
    let storage = demo_runner.get_storage();
    let is_storage_empty = storage.is_empty();
    let mut methods = get_rpc_methods::<DefaultContext>(storage);
    // register rpc methods
    {
        register_ledger(ledger_db.clone(), &mut methods)?;
        register_sequencer(da_service.clone(), &mut demo_runner, &mut methods)?;
        #[cfg(feature = "experimental")]
        register_ethereum(rollup_config.da.clone(), &mut methods)?;
    }

    let _handle = tokio::spawn(async move {
        start_rpc_server(methods, address).await;
    });

    // For demonstration, we also initialize the DaVerifier interface.
    // Running the verifier is only *necessary* during proof generation not normal execution
    let da_verifier = Arc::new(CelestiaVerifier::new(RollupParams {
        namespace: ROLLUP_NAMESPACE,
    }));

    let demo = demo_runner.inner_mut();
    let mut prev_state_root = {
        // Check if the rollup has previously been initialized
        if is_storage_empty {
            info!("No history detected. Initializing chain...");
            demo.init_chain(get_genesis_config());
            info!("Chain initialization is done.");
        } else {
            debug!("Chain is already initialized. Skipping initialization.");
        }

        // HACK: Tell the rollup that you're running an empty DA layer block so that it will return the latest state root.
        // This will be removed shortly.
        demo.begin_slot(Default::default());
        let (prev_state_root, _) = demo.end_slot();
        prev_state_root.0
    };

    // Start the main rollup loop
    let item_numbers = ledger_db.get_next_items_numbers();
    let last_slot_processed_before_shutdown = item_numbers.slot_number - 1;
    let start_height = rollup_config.start_height + last_slot_processed_before_shutdown;

    for height in start_height.. {
        info!(
            "Requesting data for height {} and prev_state_root 0x{}",
            height,
            hex::encode(prev_state_root)
        );

        // Fetch the relevant subset of the next Celestia block
        let filtered_block = da_service.get_finalized_at(height).await?;
        let header = filtered_block.header();

        // For the demo, we create and verify a proof that the data has been extracted from Celestia correctly.
        // In a production implementation, this logic would only run on the prover node - regular full nodes could
        // simply download the data from Celestia without extracting and checking a merkle proof here,
        let mut blobs = da_service.extract_relevant_txs(&filtered_block);

        info!("Received {} blobs at height {}", blobs.len(), height);

        let mut data_to_commit = SlotCommit::new(filtered_block.clone());
        demo.begin_slot(Default::default());
        for (blob_idx, blob) in blobs.iter_mut().enumerate() {
            let batch_receipt = demo.apply_blob(blob, None);
            info!(
                "blob #{} at height {} with blob_hash 0x{} has been applied with #{} transactions, sequencer outcome {:?}",
                blob_idx,
                height,
                hex::encode(batch_receipt.batch_hash),
                batch_receipt.tx_receipts.len(),
                batch_receipt.inner
            );
            for (i, tx_receipt) in batch_receipt.tx_receipts.iter().enumerate() {
                info!(
                    "tx #{} hash: 0x{} result {:?}",
                    i,
                    hex::encode(tx_receipt.tx_hash),
                    tx_receipt.receipt
                );
            }

            data_to_commit.add_batch(batch_receipt);
        }
        let (next_state_root, _witness) = demo.end_slot();

        let (inclusion_proof, completeness_proof) = da_service
            .get_extraction_proof(&filtered_block, &blobs)
            .await;

        let validity_condition = da_verifier
            .verify_relevant_tx_list::<NoOpHasher>(
                header,
                &blobs,
                inclusion_proof,
                completeness_proof,
            )
            .expect("Failed to verify relevant tx list but prover is honest");

        // For demonstration purposes, we also show how you would check the extra validity condition
        // imposed by celestia (that the Celestia block processed be the next one from the canonical chain).
        // In a real rollup, this check would only be made by light clients.
        let mut checker = CelestiaChainChecker {
            current_block_hash: *header.hash().inner(),
        };
        checker.check(&validity_condition)?;

        // Store the resulting receipts in the ledger database
        ledger_db.commit_slot(data_to_commit)?;
        prev_state_root = next_state_root.0;
    }

    Ok(())
}

fn register_sequencer(
    da_service: Arc<CelestiaService>,
    demo_runner: &mut NativeAppRunner<Risc0Verifier, BlobWithSender>,
    methods: &mut jsonrpsee::RpcModule<()>,
) -> Result<(), anyhow::Error> {
    let batch_builder = demo_runner.take_batch_builder().unwrap();
    let sequencer_rpc = get_sequencer_rpc(batch_builder, da_service);
    methods
        .merge(sequencer_rpc)
        .context("Failed to merge Txs RPC modules")
}

fn register_ledger(
    ledger_db: LedgerDB,
    methods: &mut jsonrpsee::RpcModule<()>,
) -> Result<(), anyhow::Error> {
    let ledger_rpc = ledger_rpc::get_ledger_rpc::<DemoBatchReceipt, DemoTxReceipt>(ledger_db);
    methods
        .merge(ledger_rpc)
        .context("Failed to merge ledger RPC modules")
}

#[cfg(feature = "experimental")]
fn register_ethereum(
    da_config: DaServiceConfig,
    methods: &mut jsonrpsee::RpcModule<()>,
) -> Result<(), anyhow::Error> {
    use std::fs;

    let data = fs::read_to_string(TX_SIGNER_PRIV_KEY_PATH).context("Unable to read file")?;

    let hex_key: HexKey =
        serde_json::from_str(&data).context("JSON does not have correct format.")?;

    let tx_signer_private_key = DefaultPrivateKey::from_hex(&hex_key.hex_priv_key).unwrap();

    let ethereum_rpc = get_ethereum_rpc(da_config, tx_signer_private_key);
    methods
        .merge(ethereum_rpc)
        .context("Failed to merge Ethereum RPC modules")
}
