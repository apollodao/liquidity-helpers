use cosmwasm_std::{OverflowError, StdError, Uint128};
use cw_bigint::TryFromBigIntError;
use cw_dex::CwDexError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    CwDex(#[from] CwDexError),

    #[error("{0}")]
    BigIntToU128(#[from] TryFromBigIntError<u128>),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Can't provide liquidity with more than 2 assets")]
    MoreThanTwoAssets {},

    #[error("Custom pair type not supported")]
    CustomPairType {},

    /// The minimum amount of tokens requested was not returned from the action
    #[error(
        "Did not receive expected amount of LP tokens. Expected: {min_out}, received: {received}"
    )]
    MinOutNotReceived {
        /// The minimum amount of tokens the user requested
        min_out: Uint128,
        /// The actual amount of tokens received
        received: Uint128,
    },
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
