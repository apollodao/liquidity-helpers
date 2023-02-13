use apollo_cw_asset::Asset;
use cosmwasm_std::{OverflowError, StdError, Uint128};
use cw_dex::CwDexError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    CwDex(#[from] CwDexError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("Insufficient LP tokens. Expected a minumum of {expected} but got {received}")]
    InsufficientLpTokens {
        expected: Uint128,
        received: Uint128,
    },

    #[error("Received funds don't match input assets. Expected {expected:?} but got {received:?}")]
    InputTokenMismatch {
        expected: Vec<Asset>,
        received: Vec<Asset>,
    },

    #[error("Unauthorized")]
    Unauthorized {},
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
