use apollo_cw_asset::Asset;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{to_json_binary, Addr, CosmosMsg, Env, StdResult, Uint128, WasmMsg};
use cw_dex::osmosis::OsmosisPool;
use liquidity_helper::msg::ExecuteMsg as GenericExcuteMsg;

#[cw_serde]
pub struct InstantiateMsg {}

pub type ExecuteMsg = GenericExcuteMsg<CallbackMsg>;

#[cw_serde]
pub enum CallbackMsg {
    SingleSidedJoin {
        asset: Asset,
        pool: OsmosisPool,
    },
    ReturnLpTokens {
        pool: OsmosisPool,
        balance_before: Uint128,
        recipient: Addr,
        min_out: Uint128,
    },
}

impl CallbackMsg {
    pub fn into_cosmos_msg(&self, env: &Env) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_json_binary(&ExecuteMsg::Callback(self.clone()))?,
            funds: vec![],
        }))
    }
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}

#[cw_serde]
pub struct MigrateMsg {}
