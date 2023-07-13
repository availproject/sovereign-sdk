use anyhow::Result;
use avail_subxt::{
	api::{
		self,
		runtime_types::{
			da_control::pallet::Call as DaCall, sp_core::bounded::bounded_vec::BoundedVec,
		},
	},
	avail::AppUncheckedExtrinsic,
	build_client,
	primitives::AvailExtrinsicParams,
	Call
};
use sp_keyring::AccountKeyring;
use structopt::StructOpt;
use subxt::tx::PairSigner;
use serde_json::to_vec;
use serde::{Deserialize, Serialize};
use sp_keyring::sr25519::sr25519::{self, Pair};
use sp_core::crypto::Pair as PairTrait;
use avail_subxt::AvailConfig;
use std::str::FromStr;
use std::fs;

const SEED_PHRASE: &str =
	"rose label choose orphan garlic upset scout payment first have boil stamp";

#[derive(Debug)]
struct HexData(Vec<u8>);

impl FromStr for HexData {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        hex::decode(s).map(HexData)
    }
}

#[derive(Debug, StructOpt)]
struct Opts {
    	/// The WebSocket address of the target the Avail Node,
	#[structopt(name = "ws_uri", long, default_value = "ws://127.0.0.1:9944")]
	pub ws: String,

	/// Check whether the Client you are using is aligned with the statically generated codegen.
	#[structopt(name = "validate_codege", short = "c", long)]
	pub validate_codegen: bool,
    

	#[structopt(name = "app_id", long, default_value = "0")]
	pub app_id: u32
}

/// This example submits an Avail data extrinsic, then retrieves the block containing the
/// extrinsic and matches the data.
#[async_std::main]
async fn main() -> Result<()> {
	let args = Opts::from_args();
	println!("{}", args.ws);

    let pair = Pair::from_phrase(SEED_PHRASE, None).unwrap();
	let signer = PairSigner::<AvailConfig, sr25519::Pair>::new(pair.0.clone());
	println!("{}", pair.0.clone().public());

	let client = build_client(args.ws, args.validate_codegen).await?;
	let example_data = args.tx_blob.0;
    let app_id = match args.app_id {
        0 => {
            let query = api::storage().data_availability().next_app_id();
            let next_app_id = client.storage().at(None).await?.fetch(&query).await?.unwrap();
            let create_application_key = api::tx()
                .data_availability()
                .create_application_key(BoundedVec(next_app_id.0.to_le_bytes().to_vec()));

            let params = AvailExtrinsicParams::default();

            let res = client
                .tx()
                .sign_and_submit_then_watch(&create_application_key, &signer, params)
                .await?
                .wait_for_finalized_success()
                .await?;

            next_app_id.0
        }

        _ => args.app_id
    };

    println!("app id is: {}", app_id);

    let data_transfer = api::tx()
		.data_availability()
		.submit_data(BoundedVec(example_data.clone()));
	let extrinsic_params = AvailExtrinsicParams::new_with_app_id(app_id.into());

	let h = client
		.tx()
		.sign_and_submit_then_watch(&data_transfer, &signer, extrinsic_params)
		.await?
		.wait_for_finalized_success()
		.await?;

	println!("receipt {:#?}", h.extrinsic_hash());

	let submitted_block = client.rpc().block(Some(h.block_hash())).await?.unwrap();

	let matched_xt = submitted_block
		.block
		.extrinsics
		.into_iter()
		.filter_map(|chain_block_ext| {
			AppUncheckedExtrinsic::try_from(chain_block_ext)
				.map(|ext| ext.function)
				.ok()
		})
		.find(|call| match call {
			Call::DataAvailability(da_call) => match da_call {
				DaCall::submit_data { data } => data.0 == example_data,
				_ => false,
			},
			_ => false,
		});

	assert!(matched_xt.is_some(), "Submitted data not found");

	Ok(())
}
