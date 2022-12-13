use apollo_utils::assets::separate_natives_and_cw20s;
use cw20::Cw20ExecuteMsg;
use cw_asset::AssetList;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    to_binary, Addr, Api, Binary, Coin, CosmosMsg, Empty, StdResult, Uint128, WasmMsg,
};

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

    pub fn call<C: Serialize, T: Into<ExecuteMsg<C>>>(
        &self,
        msg: T,
        funds: Vec<Coin>,
    ) -> StdResult<CosmosMsg> {
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
        recipient: Option<String>,
    ) -> StdResult<Vec<CosmosMsg>> {
        let (funds, cw20s) = separate_natives_and_cw20s(&assets);

        // Increase allowance for all cw20s
        let mut msgs: Vec<CosmosMsg> = cw20s
            .into_iter()
            .map(|asset| {
                Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: asset.address,
                    msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                        spender: self.addr().into(),
                        amount: asset.amount,
                        expires: None,
                    })?,
                    funds: vec![],
                }))
            })
            .collect::<StdResult<Vec<_>>>()?;

        msgs.push(self.call(
            ExecuteMsg::<Empty>::BalancingProvideLiquidity {
                assets: assets.into(),
                min_out,
                pool,
                recipient,
            },
            funds,
        )?);

        Ok(msgs)
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

impl<T> From<T> for LiquidityHelperBase<T> {
    fn from(x: T) -> Self {
        LiquidityHelperBase(x)
    }
}
