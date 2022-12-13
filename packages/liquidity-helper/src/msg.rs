use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Binary, Uint128};
use cw_asset::AssetListUnchecked;

#[cw_serde]
pub enum ExecuteMsg<C> {
    BalancingProvideLiquidity {
        assets: AssetListUnchecked,
        min_out: Uint128,
        pool: Binary,
        recipient: Option<String>,
    },
    Callback(C),
}
