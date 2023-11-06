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

    #[error("{0}")]
    Semver(#[from] semver::Error),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Can't provide liquidity with more than 2 assets")]
    MoreThanTwoAssets {},

    #[error("Pair type not supported")]
    UnsupportedPairType {},

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

    #[error("Can only migrate to a codeID with the correct name. Expected: {expected}, received: {received}")]
    InvalidContractName {
        /// The expected contract name
        expected: String,
        /// The actual contract name
        received: String,
    },

    #[error("Can only migrate to a codeID with a newer version. Old version: {old_version}, new version: {new_version}")]
    InvalidContractVersion {
        /// The current contract version
        old_version: semver::Version,
        /// The version that the user is trying to migrate to
        new_version: semver::Version,
    },
}

impl From<ContractError> for StdError {
    fn from(e: ContractError) -> Self {
        StdError::generic_err(e.to_string())
    }
}
