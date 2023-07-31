use std::fmt::Display;
use std::str::FromStr;

use demo_simple_stf::{ApplyBlobResult, CheckHashPreimageStf};
use sov_rollup_interface::mocks::{MockZkvm, TestBlob};
use sov_rollup_interface::stf::StateTransitionFunction;
use sov_rollup_interface::AddressTrait;

#[derive(PartialEq, Debug, Clone, Eq, serde::Serialize, serde::Deserialize, Hash)]
pub struct DaAddress {
    pub addr: [u8; 32],
}

impl AddressTrait for DaAddress {}

impl AsRef<[u8]> for DaAddress {
    fn as_ref(&self) -> &[u8] {
        &self.addr
    }
}

impl From<[u8; 32]> for DaAddress {
    fn from(addr: [u8; 32]) -> Self {
        DaAddress { addr }
    }
}

impl FromStr for DaAddress {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Remove the "0x" prefix, if it exists.
        let s = s.strip_prefix("0x").unwrap_or(s);
        let mut addr = [0u8; 32];
        hex::decode_to_slice(s, &mut addr)?;
        Ok(DaAddress { addr })
    }
}

impl<'a> TryFrom<&'a [u8]> for DaAddress {
    type Error = anyhow::Error;

    fn try_from(addr: &'a [u8]) -> Result<Self, Self::Error> {
        if addr.len() != 32 {
            anyhow::bail!("Address must be 32 bytes long");
        }
        let mut addr_bytes = [0u8; 32];
        addr_bytes.copy_from_slice(addr);
        Ok(Self { addr: addr_bytes })
    }
}

impl Display for DaAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.addr)
    }
}

#[test]
fn test_stf() {
    let address = DaAddress { addr: [1; 32] };
    let preimage = vec![0; 32];

    let mut test_blob = TestBlob::<DaAddress>::new(preimage, address, [0; 32]);
    let stf = &mut CheckHashPreimageStf {};

    StateTransitionFunction::<MockZkvm, TestBlob<DaAddress>>::init_chain(stf, ());
    StateTransitionFunction::<MockZkvm, TestBlob<DaAddress>>::begin_slot(stf, ());

    let receipt = StateTransitionFunction::<MockZkvm, TestBlob<DaAddress>>::apply_blob(
        stf,
        &mut test_blob,
        None,
    );
    assert_eq!(receipt.inner, ApplyBlobResult::Success);

    StateTransitionFunction::<MockZkvm, TestBlob<DaAddress>>::end_slot(stf);
}
