use anyhow::Result;
use sov_rollup_interface::zk::Zkvm;
use sov_state::WorkingSet;

use crate::ProverIncentives;

impl<C: sov_modules_api::Context, Vm: Zkvm> ProverIncentives<C, Vm> {
    pub(crate) fn init_module(
        &self,
        config: &<Self as sov_modules_api::Module>::Config,
        working_set: &mut WorkingSet<C::Storage>,
    ) -> Result<()> {
        anyhow::ensure!(
            !config.initial_provers.is_empty(),
            "At least one prover must be set at genesis!"
        );

        self.minimum_bond.set(&config.minimum_bond, working_set);
        self.commitment_of_allowed_verifier_method.set(
            &crate::StoredCodeCommitment {
                commitment: config.commitment_of_allowed_verifier_method.clone(),
            },
            working_set,
        );
        self.bonding_token_address
            .set(&config.bonding_token_address, working_set);

        for (prover, bond) in config.initial_provers.iter() {
            self.bond_prover_helper(*bond, prover, working_set)?;
        }

        Ok(())
    }
}
