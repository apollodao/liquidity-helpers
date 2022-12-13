//! # Osmosis Liquidity Helper
//!
//! This contract helps provide liquidity for Osmosis pools and supports
//! supplying liquidity with imbalanced assets. If the assets provided are not
//! in the correct ratio, the contract will swap some of the assets so that the
//! ratio of assets are the same as the pools reserves after the swap.

pub mod contract;
mod error;
pub mod msg;

pub use crate::error::ContractError;
