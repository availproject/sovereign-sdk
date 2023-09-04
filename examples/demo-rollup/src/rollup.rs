use std::net::SocketAddr;
use std::str::FromStr;

use anyhow::Context;
use celestia::verifier::address::CelestiaAddress;
use celestia::verifier::RollupParams;
use celestia::CelestiaService;
use const_rollup_config::SEQUENCER_DA_ADDRESS;
#[cfg(feature = "experimental")]
use demo_stf::app::DefaultPrivateKey;
use demo_stf::app::{App, DefaultContext};
use demo_stf::runtime::{get_rpc_methods, GenesisConfig};
use risc0_adapter::host::Risc0Verifier;
use sov_db::ledger_db::LedgerDB;
#[cfg(feature = "experimental")]
use sov_ethereum::experimental::EthRpcConfig;
use sov_rollup_interface::services::da::DaService;
use sov_rollup_interface::zk::Zkvm;
use sov_state::storage::Storage;
use sov_stf_runner::{from_toml_path, RollupConfig, RunnerConfig, StateTransitionRunner};
use tokio::sync::oneshot;
use tracing::debug;

#[cfg(feature = "experimental")]
use crate::register_rpc::register_ethereum;
use crate::register_rpc::{register_ledger, register_sequencer};
use crate::{get_genesis_config, initialize_ledger, ROLLUP_NAMESPACE};

#[cfg(feature = "experimental")]
const TX_SIGNER_PRIV_KEY_PATH: &str = "../test-data/keys/tx_signer_private_key.json";

/// Dependencies needed to run the rollup.
pub struct Rollup<Vm: Zkvm, Da: DaService + Clone> {
    /// Implementation of the STF.
    pub app: App<Vm, Da::Spec>,
    /// Data availability service.
    pub da_service: Da,
    /// Ledger db.
    pub ledger_db: LedgerDB,
    /// Runner configuration.
    pub runner_config: RunnerConfig,
    /// Initial rollup configuration.
    pub genesis_config: GenesisConfig<DefaultContext, Da::Spec>,
    #[cfg(feature = "experimental")]
    /// Configuration for the Ethereum RPC.
    pub eth_rpc_config: EthRpcConfig,
}

/// Creates celestia based rollup.
pub async fn new_rollup_with_celestia_da(
    rollup_config_path: &str,
) -> Result<Rollup<Risc0Verifier, CelestiaService>, anyhow::Error> {
    debug!("Starting demo rollup with config {}", rollup_config_path);
    let rollup_config: RollupConfig<celestia::DaServiceConfig> =
        from_toml_path(rollup_config_path).context("Failed to read rollup configuration")?;

    let ledger_db = initialize_ledger(&rollup_config.storage.path);

    let da_service = CelestiaService::new(
        rollup_config.da.clone(),
        RollupParams {
            namespace: ROLLUP_NAMESPACE,
        },
    )
    .await;

    let app = App::new(rollup_config.storage);
    let sequencer_da_address = CelestiaAddress::from_str(SEQUENCER_DA_ADDRESS)?;
    let genesis_config = get_genesis_config(sequencer_da_address);

    Ok(Rollup {
        app,
        da_service,
        ledger_db,
        runner_config: rollup_config.runner,
        genesis_config,
        #[cfg(feature = "experimental")]
        eth_rpc_config: EthRpcConfig {
            min_blob_size: Some(1),
            tx_signer_priv_key: read_tx_signer_priv_key()?,
        },
    })
}

#[cfg(feature = "experimental")]
/// Ethereum RPC wraps EVM transaction in a rollup transaction.
/// This function reads the private key of the rollup transaction signer.
pub fn read_tx_signer_priv_key() -> Result<DefaultPrivateKey, anyhow::Error> {
    let data = std::fs::read_to_string(TX_SIGNER_PRIV_KEY_PATH).context("Unable to read file")?;

    let hex_key: crate::HexPrivateAndAddress =
        serde_json::from_str(&data).context("JSON does not have correct format.")?;

    let priv_key = sov_modules_api::default_signature::private_key::DefaultPrivateKey::from_hex(
        &hex_key.hex_priv_key,
    )?;

    Ok(priv_key)
}

impl<Vm: Zkvm, Da: DaService<Error = anyhow::Error> + Clone> Rollup<Vm, Da> {
    /// Runs the rollup.
    pub async fn run(self) -> Result<(), anyhow::Error> {
        self.run_and_report_rpc_port(None).await
    }

    /// Runs the rollup. Reports rpc port to the caller using the provided channel.
    pub async fn run_and_report_rpc_port(
        mut self,
        channel: Option<oneshot::Sender<SocketAddr>>,
    ) -> Result<(), anyhow::Error> {
        let storage = self.app.get_storage();
        let mut methods = get_rpc_methods::<DefaultContext, Da::Spec>(storage);

        // register rpc methods
        {
            register_ledger(self.ledger_db.clone(), &mut methods)?;
            register_sequencer(self.da_service.clone(), &mut self.app, &mut methods)?;
            #[cfg(feature = "experimental")]
            register_ethereum(self.da_service.clone(), self.eth_rpc_config, &mut methods)?;
        }

        let storage = self.app.get_storage();

        let mut runner = StateTransitionRunner::new(
            self.runner_config,
            self.da_service,
            self.ledger_db,
            self.app.stf,
            storage.is_empty(),
            self.genesis_config,
        )?;

        runner.start_rpc_server(methods, channel).await;
        runner.run().await?;

        Ok(())
    }
}
