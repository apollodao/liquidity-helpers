use cw_asset::AssetList;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{to_binary, Addr, Binary, CosmosMsg, StdResult, Uint128, WasmMsg};

use crate::msg::ExecuteMsg;

/// LiquidityHelper is a wrapper around Addr that provides a lot of helpers
/// for working with this contract. It can be imported by other contracts
/// who wish to call this contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct LiquidityHelper(pub Addr);

impl LiquidityHelper {
    pub fn addr(&self) -> Addr {
        self.0.clone()
    }

    pub fn call<T: Into<ExecuteMsg>>(&self, msg: T) -> StdResult<CosmosMsg> {
        let msg = to_binary(&msg.into())?;
        Ok(WasmMsg::Execute {
            contract_addr: self.addr().into(),
            msg,
            funds: vec![],
        }
        .into())
    }

    pub fn balancing_provide_liquidity(
        &self,
        assets: AssetList,
        min_out: Uint128,
        pool: Binary,
    ) -> StdResult<CosmosMsg> {
        self.call(ExecuteMsg::BalancingProvideLiquidity {
            assets: assets.into(),
            min_out,
            pool,
        })
    }
}
