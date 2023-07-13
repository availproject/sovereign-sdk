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

	#[structopt(name = "app_name", long, default_value = "example")]
	pub app_name: String
}

#[derive(Serialize, Deserialize)]
struct AppIdResult {
	app_id: u32
}

/// This example submits an Avail data extrinsic, then retrieves the block containing the
/// extrinsic and matches the data.
#[async_std::main]
async fn main() -> Result<()> {
	let args = Opts::from_args();

	let pair = Pair::from_phrase(SEED_PHRASE, None).unwrap();
	let signer = PairSigner::<AvailConfig, sr25519::Pair>::new(pair.0.clone());

	let client = build_client(args.ws, args.validate_codegen).await?;

	let app_id = {
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
	};
	
	println!("{}", serde_json::to_string(
		&AppIdResult {
			app_id
		}
	).unwrap());

	Ok(())
}
