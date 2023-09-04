//! Defines the query methods for the attester incentives module
use serde::{Deserialize, Serialize};
use sov_modules_api::Spec;
use sov_rollup_interface::da::DaSpec;
use sov_rollup_interface::zk::{ValidityConditionChecker, Zkvm};
use sov_state::storage::{NativeStorage, StorageProof};
use sov_state::{Storage, WorkingSet};

use super::AttesterIncentives;
use crate::call::Role;

/// The response type to the `getBondAmount` query.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct BondAmountResponse {
    /// The value of the bond
    pub value: u64,
}

// TODO: implement rpc_gen macro
impl<C, Vm, Da, Checker> AttesterIncentives<C, Vm, Da, Checker>
where
    C: sov_modules_api::Context,
    Vm: Zkvm,
    Da: DaSpec,
    Checker: ValidityConditionChecker<Da::ValidityCondition>,
{
    /// Queries the state of the module.
    pub fn get_bond_amount(
        &self,
        address: C::Address,
        role: Role,
        working_set: &mut WorkingSet<C::Storage>,
    ) -> BondAmountResponse {
        match role {
            Role::Attester => BondAmountResponse {
                value: self
                    .bonded_attesters
                    .get(&address, working_set)
                    .unwrap_or_default(),
            },
            Role::Challenger => BondAmountResponse {
                value: self
                    .bonded_challengers
                    .get(&address, working_set)
                    .unwrap_or_default(),
            },
        }
    }

    /// Used by attesters to get a proof that they were bonded before starting to produce attestations.
    /// A bonding proof is valid for `max_finality_period` blocks, the attester can only produce transition
    /// attestations for this specific amount of time.
    pub fn get_bond_proof(
        &self,
        address: C::Address,
        witness: &<<C as Spec>::Storage as Storage>::Witness,
        working_set: &mut WorkingSet<C::Storage>,
    ) -> StorageProof<<C::Storage as Storage>::Proof>
    where
        C::Storage: NativeStorage,
    {
        working_set.backing().get_with_proof_from_state_map(
            &address,
            &self.bonded_attesters,
            witness,
        )
    }

    /// TODO: Make the unbonding amount queryable:
    pub fn get_unbonding_amount(
        &self,
        _address: C::Address,
        _witness: &<<C as Spec>::Storage as Storage>::Witness,
        _working_set: &mut WorkingSet<C::Storage>,
    ) -> u64 {
        todo!("Make the unbonding amount queryable: https://github.com/Sovereign-Labs/sovereign-sdk/issues/675")
    }
}
