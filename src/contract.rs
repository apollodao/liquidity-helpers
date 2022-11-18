use apollo_utils::assets::assert_only_native_coins;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw_asset::{Asset, AssetList};
use cw_dex::osmosis::OsmosisPool;
use cw_dex::traits::Pool;

use crate::error::ContractError;
use crate::msg::{CallbackMsg, ExecuteMsg, InstantiateMsg, QueryMsg};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:osmosis-liquidity-helper";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::BalancingProvideLiquidity {
            assets,
            min_out,
            pool,
        } => {
            let assets = assets.check(deps.api)?;
            let pool: OsmosisPool = from_binary(&pool)?;
            execute_balancing_provide_liquidity(deps, env, info, assets, min_out, pool)
        }
        ExecuteMsg::Callback(msg) => {
            // Only contract can call callbacks
            if info.sender != env.contract.address {
                return Err(ContractError::Unauthorized {});
            }

            match msg {
                CallbackMsg::SingleSidedJoin { asset, pool } => {
                    execute_callback_single_sided_join(deps, env, info, asset, pool)
                }
            }
        }
    }
}

pub fn execute_balancing_provide_liquidity(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    mut assets: AssetList,
    min_out: Uint128,
    pool: OsmosisPool,
) -> Result<Response, ContractError> {
    // Assert that only native coins are sent
    assert_only_native_coins(&assets)?;

    if assets.len() == 1 {
        // Provide single sided
        Ok(pool.provide_liquidity(deps.as_ref(), &env, assets, min_out)?)
    } else {
        // Provide as much as possible double sided, and then issue callbacks to
        // provide the remainder single sided
        let (lp_tokens_received, tokens_used) =
            pool.simulate_noswap_join(&deps.querier, &assets)?;

        // Get response with msg to provide double sided
        let mut response =
            pool.provide_liquidity(deps.as_ref(), &env, assets.clone(), lp_tokens_received)?;

        // Deduct tokens used to get remaining tokens
        assets.deduct_many(&tokens_used)?;

        // For each of the remaining tokens, issue a callback to provide
        // liquidity single sided
        for asset in assets.into_iter() {
            if asset.amount > Uint128::zero() {
                let msg = CallbackMsg::SingleSidedJoin {
                    asset: asset.clone(),
                    pool: pool.clone(),
                }
                .into_cosmos_msg(&env)?;
                response = response.add_message(msg);
            }
        }

        Ok(response)
    }
}

/// CallbackMsg handler to provide liquidity with the given assets. This needs
/// to be a callback, rather than doing in the first ExecuteMsg, because
/// pool.provide_liquidity does a simulation with current reserves, and our
/// actions in the top level ExecuteMsg will alter the reserves, which means the
/// reserves would be wrong in the provide liquidity simulation.
pub fn execute_callback_single_sided_join(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    asset: Asset,
    pool: OsmosisPool,
) -> Result<Response, ContractError> {
    let assets = AssetList::from(vec![asset]);
    let res = pool.provide_liquidity(deps.as_ref(), &env, assets, Uint128::one())?;
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!();
}

#[cfg(test)]
mod tests {}
