use apollo_utils::assets::separate_natives_and_cw20s;
use cw_asset::AssetList;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{to_binary, Addr, Api, Binary, Coin, CosmosMsg, StdResult, Uint128, WasmMsg};

use crate::msg::ExecuteMsg;

/// LiquidityHelper is a wrapper around Addr that provides a lot of helpers
/// for working with this contract. It can be imported by other contracts
/// who wish to call this contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct LiquidityHelperBase<T>(pub T);

pub type LiquidityHelperUnchecked = LiquidityHelperBase<String>;
pub type LiquidityHelper = LiquidityHelperBase<Addr>;

impl LiquidityHelper {
    pub fn new(address: Addr) -> Self {
        Self(address)
    }

    pub fn addr(&self) -> Addr {
        self.0.clone()
    }

    pub fn call<T: Into<ExecuteMsg>>(&self, msg: T, funds: Vec<Coin>) -> StdResult<CosmosMsg> {
        let msg = to_binary(&msg.into())?;
        Ok(WasmMsg::Execute {
            contract_addr: self.addr().into(),
            msg,
            funds,
        }
        .into())
    }

    pub fn balancing_provide_liquidity(
        &self,
        assets: AssetList,
        min_out: Uint128,
        pool: Binary,
    ) -> StdResult<CosmosMsg> {
        let (funds, _) = separate_natives_and_cw20s(&assets);
        self.call(
            ExecuteMsg::BalancingProvideLiquidity {
                assets: assets.into(),
                min_out,
                pool,
            },
            funds,
        )
    }
}

impl LiquidityHelperUnchecked {
    pub fn new(addr: String) -> Self {
        Self(addr)
    }

    pub fn check(&self, api: &dyn Api) -> StdResult<LiquidityHelper> {
        Ok(LiquidityHelperBase(api.addr_validate(&self.0)?))
    }
}

impl From<LiquidityHelper> for LiquidityHelperUnchecked {
    fn from(h: LiquidityHelper) -> Self {
        LiquidityHelperBase(h.0.to_string())
    }
}
