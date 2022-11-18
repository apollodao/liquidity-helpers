use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{to_binary, Binary, CosmosMsg, Env, StdResult, Uint128, WasmMsg};
use cw_asset::{Asset, AssetListUnchecked};
use cw_dex::osmosis::OsmosisPool;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    BalancingProvideLiquidity {
        assets: AssetListUnchecked,
        min_out: Uint128,
        pool: Binary,
    },
    Callback(CallbackMsg),
}

#[cw_serde]
pub enum CallbackMsg {
    SingleSidedJoin { asset: Asset, pool: OsmosisPool },
}

impl CallbackMsg {
    pub fn into_cosmos_msg(&self, env: &Env) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(self.clone()))?,
            funds: vec![],
        }))
    }
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}
