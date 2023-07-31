mod call;
mod genesis;
#[cfg(feature = "native")]
mod query;
mod token;
mod utils;

pub use call::CallMessage;
#[cfg(feature = "native")]
pub use query::{BalanceResponse, BankRpcImpl, BankRpcServer, TotalSupplyResponse};
use sov_modules_api::{Error, ModuleInfo};
use sov_state::WorkingSet;
use token::Token;
pub use token::{Amount, Coins};
pub use utils::{get_genesis_token_address, get_token_address};

pub struct TokenConfig<C: sov_modules_api::Context> {
    pub token_name: String,
    pub address_and_balances: Vec<(C::Address, u64)>,
    pub authorized_minters: Vec<C::Address>,
    pub salt: u64,
}

/// Initial configuration for sov-bank module.
pub struct BankConfig<C: sov_modules_api::Context> {
    pub tokens: Vec<TokenConfig<C>>,
}

/// The sov-bank module manages user balances. It provides functionality for:
/// - Token creation.
/// - Token transfers.
/// - Token burn.
#[cfg_attr(feature = "native", derive(sov_modules_api::ModuleCallJsonSchema))]
#[derive(ModuleInfo, Clone)]
pub struct Bank<C: sov_modules_api::Context> {
    /// The address of the sov-bank module.
    #[address]
    pub(crate) address: C::Address,

    /// A mapping of addresses to tokens in the sov-bank.
    #[state]
    pub(crate) tokens: sov_state::StateMap<C::Address, Token<C>>,
}

impl<C: sov_modules_api::Context> sov_modules_api::Module for Bank<C> {
    type Context = C;

    type Config = BankConfig<C>;

    type CallMessage = call::CallMessage<C>;

    fn genesis(
        &self,
        config: &Self::Config,
        working_set: &mut WorkingSet<C::Storage>,
    ) -> Result<(), Error> {
        Ok(self.init_module(config, working_set)?)
    }

    fn call(
        &self,
        msg: Self::CallMessage,
        context: &Self::Context,
        working_set: &mut WorkingSet<C::Storage>,
    ) -> Result<sov_modules_api::CallResponse, Error> {
        match msg {
            call::CallMessage::CreateToken {
                salt,
                token_name,
                initial_balance,
                minter_address,
                authorized_minters,
            } => Ok(self.create_token(
                token_name,
                salt,
                initial_balance,
                minter_address,
                authorized_minters,
                context,
                working_set,
            )?),

            call::CallMessage::Transfer { to, coins } => {
                Ok(self.transfer(to, coins, context, working_set)?)
            }

            call::CallMessage::Burn { coins } => Ok(self.burn(coins, context, working_set)?),

            call::CallMessage::Mint {
                coins,
                minter_address,
            } => Ok(self.mint(coins, minter_address, context, working_set)?),

            call::CallMessage::Freeze { token_address } => {
                Ok(self.freeze(token_address, context, working_set)?)
            }
        }
    }
}
