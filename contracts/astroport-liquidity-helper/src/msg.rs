use apollo_cw_asset::AssetList;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{to_binary, Addr, CosmosMsg, Env, StdResult, Uint128, WasmMsg};
use cw_dex::astroport::AstroportPool;
use liquidity_helper::msg::ExecuteMsg as GenericExecuteMsg;

#[cw_serde]
pub struct InstantiateMsg {
    pub astroport_factory: String,
}

pub type ExecuteMsg = GenericExecuteMsg<CallbackMsg>;

#[cw_serde]
pub enum CallbackMsg {
    ProvideLiquidity {
        assets: AssetList,
        min_out: Uint128,
        pool: AstroportPool,
    },
    ReturnLpTokens {
        pool: AstroportPool,
        balance_before: Uint128,
        recipient: Addr,
    },
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
pub enum QueryMsg {
    #[returns(Addr)]
    AstroportFactory {},
}

#[cw_serde]
pub enum MigrateMsg {}
