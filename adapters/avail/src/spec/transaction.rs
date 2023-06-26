use super::address::AvailAddress;
use avail_subxt::{
    api::runtime_types::{da_control::pallet::Call, da_runtime::RuntimeCall::DataAvailability},
    primitives::AppUncheckedExtrinsic,
};
use subxt::ext::parity_scale_codec::Encode;

use serde::{Deserialize, Serialize};
use sov_rollup_interface::{da::{BlobTransactionTrait, CountedBufReader}, Bytes};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AvailBlobTransaction(pub AppUncheckedExtrinsic);

impl BlobTransactionTrait for AvailBlobTransaction {
    type Data = Bytes;

    type Address = AvailAddress;

    fn sender(&self) -> AvailAddress {
        match &self.0.signature {
            Some((subxt::utils::MultiAddress::Id(id), _, _)) => AvailAddress(id.clone().0),
            _ => unimplemented!(),
        }
    }

    fn data(&self) -> &CountedBufReader<Self::Data> {
        match &self.0.function {
            DataAvailability(Call::submit_data { data }) => &CountedBufReader::<Bytes>::new(Bytes::copy_from_slice(&data.0)),
            _ => unimplemented!(),
        }
    }

    fn data_mut(&mut self) -> &mut CountedBufReader<Self::Data> {
        match &self.0.function {
            DataAvailability(Call::submit_data { data }) => &mut CountedBufReader::<Bytes>::new(Bytes::copy_from_slice(&data.0)),
            _ => unimplemented!(),
        }
    }

    fn hash(&self) -> [u8; 32] {
        self.0.encode()
    }
}
