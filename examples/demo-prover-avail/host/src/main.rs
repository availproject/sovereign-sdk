use std::env;
use std::str::FromStr;

use anyhow::Context;
use const_rollup_config::{ROLLUP_NAMESPACE_RAW, SEQUENCER_DA_ADDRESS};
use demo_stf::app::{App, DefaultPrivateKey};
use demo_stf::genesis_config::create_demo_genesis_config;
use methods::{ROLLUP_ELF, ROLLUP_ID};
use risc0_adapter::host::{Risc0Host, Risc0Verifier};
use sov_modules_api::PrivateKey;
use sov_rollup_interface::services::da::DaService;
use sov_rollup_interface::stf::StateTransitionFunction;
use sov_rollup_interface::zk::ZkvmHost;
use sov_state::Storage;
use sov_stf_runner::{from_toml_path, RollupConfig};
use tracing::{info, Level};

use presence::service::DaProvider as AvailDaProvider;
use presence::spec::transaction::AvailBlobTransaction;
use presence::spec::DaLayerSpec;

pub fn get_genesis_config(sequencer_da_address: &str) -> GenesisConfig<DefaultContext, DaLayerSpec> {
    let sequencer_private_key = DefaultPrivateKey::generate();
    
    create_demo_genesis_config(
        100000000,
        sequencer_private_key.default_address(),
        sequencer_da_address.as_ref().to_vec(),
        &sequencer_private_key,
        &sequencer_private_key,
    )
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Initializing logging
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_err| eprintln!("Unable to set global default subscriber"))
        .expect("Cannot fail to set subscriber");

    let rollup_config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "rollup_config.toml".to_string());
    let rollup_config: RollupConfig<AvailServiceConfig> =
        from_toml_path(rollup_config_path).context("Failed to read rollup configuration")?;

   let da_service = AvailService::new(
        rollup_config.da.clone()
    )
    .await;

    let app: App<Risc0Verifier, DaLayerSpec> = App::new(rollup_config.storage);

    let is_storage_empty = app.get_storage().is_empty();
    let mut demo = app.stf;

    let mut prev_state_root = {
        // Check if the rollup has previously been initialized
        if is_storage_empty {
            info!("No history detected. Initializing chain...");
            demo.init_chain(get_genesis_config(&SEQUENCER_AVAIL_DA_ADDRESS));
            info!("Chain initialization is done.");
        } else {
            info!("Chain is already initialized. Skipping initialization");
        }

        let res = demo.apply_slot(Default::default(), []);
        res.state_root.0
    };

    //TODO: Start from slot processed before shut down.

    for height in config.rollup_config.start_height..=config.rollup_config.start_height + 30 {
        let mut host = Risc0Host::new(ROLLUP_ELF);
        host.write_to_guest(prev_state_root);

        info!(
            "Requesting data for height {} and prev_state_root 0x{}",
            height,
            hex::encode(prev_state_root)
        );
        let filtered_block = da_service.get_finalized_at(height).await?;
        let header_hash = hex::encode(filtered_block.hash());
        host.write_to_guest(&filtered_block.header);
        let (mut blob_txs, inclusion_proof, completeness_proof) =
            da_service.extract_relevant_txs_with_proof(&filtered_block).await;

        info!(
            "Extracted {} relevant blobs at height {} header 0x{}",
            blob_txs.len(),
            height,
            header_hash,
        );

        host.write_to_guest(&inclusion_proof);
        host.write_to_guest(&completeness_proof);
        host.write_to_guest(&blob_txs);

        let result = demo.apply_slot(Default::default(), &mut blob_txs);

        host.write_to_guest(&result.witness);

        info!("Starting proving...");
        let receipt = host.run().expect("Prover should run successfully");
        info!("Start verifying..");
        receipt.verify(ROLLUP_ID).expect("Receipt should be valid");

        prev_state_root = result.state_root.0;
        info!("Completed proving and verifying block {height}");
    }

    Ok(())
}
