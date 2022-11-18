use cosmwasm_std::StdError;
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

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Can't provide liquidity with more than 2 assets")]
    MoreThanTwoAssets {},

    #[error("Custom pair type not supported")]
    CustomPairType {},
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
