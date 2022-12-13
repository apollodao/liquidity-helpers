use apollo_utils::assets::receive_assets;
use apollo_utils::responses::merge_responses;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, Event, MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw_asset::{Asset, AssetList};
use cw_dex::astroport::helpers::query_fee_info;
use cw_dex::astroport::msg::PairType;
use cw_dex::astroport::AstroportPool;
use cw_dex::traits::Pool;

use crate::error::ContractError;
use crate::math::calc_xyk_balancing_swap;
use crate::msg::{CallbackMsg, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::ASTROPORT_FACTORY;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:astroport-liquidity-helper";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let astroport_factory = deps.api.addr_validate(&msg.astroport_factory)?;
    ASTROPORT_FACTORY.save(deps.storage, &astroport_factory)?;

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
            recipient,
        } => {
            let assets = assets.check(deps.api)?;
            let pool: AstroportPool = from_binary(&pool)?;
            execute_balancing_provide_liquidity(deps, env, info, assets, min_out, pool, recipient)
        }
        ExecuteMsg::Callback(msg) => {
            // Only contract can call callbacks
            if info.sender != env.contract.address {
                return Err(ContractError::Unauthorized {});
            }

            match msg {
                CallbackMsg::ProvideLiquidity {
                    assets,
                    min_out,
                    pool,
                } => execute_callback_provide_liquidity(
                    deps, env, info, assets, min_out, pool,
                ),
                CallbackMsg::ReturnLpTokens {
                    pool,
                    balance_before,
                    recipient,
                } => execute_callback_return_lp_tokens(
                    deps,
                    env,
                    info,
                    pool,
                    balance_before,
                    recipient,
                ),
            }
        }
    }
}

pub fn execute_balancing_provide_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mut assets: AssetList,
    min_out: Uint128,
    pool: AstroportPool,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    // Get response with message to do TransferFrom on any Cw20s and assert that
    // native tokens have been received already.
    let receive_res = receive_assets(&info, &env, &assets)?;

    // Unwrap recipient or use caller's address
    let recipient = recipient.map_or(Ok(info.sender), |x| deps.api.addr_validate(&x))?;

    // Check lp token balance before, to pass into callback
    let lp_token_balance = pool
        .lp_token()
        .query_balance(&deps.querier, env.contract.address.to_string())?;

    match pool.pair_type {
        PairType::Xyk {} => {
            // For XYK pools we need to swap some amount of one asset
            // into the other and then provide liquidity
            let pool_res = pool.query_pool_info(&deps.querier)?;

            let pool_reserves: [Asset; 2] =
                [(&pool_res.assets[0]).into(), (&pool_res.assets[1]).into()];
            if assets.len() > 2 {
                return Err(ContractError::MoreThanTwoAssets {});
            }

            // If only one asset is provided in the AssetList, we need to
            // create the other asset with an empty amount
            let assets_slice: [Asset; 2] = [
                assets
                    .find(&pool_reserves[0].info)
                    .cloned()
                    .unwrap_or_else(|| Asset {
                        info: pool_reserves[0].info.clone(),
                        amount: Uint128::zero(),
                    }),
                assets
                    .find(&pool_reserves[1].info)
                    .cloned()
                    .unwrap_or_else(|| Asset {
                        info: pool_reserves[1].info.clone(),
                        amount: Uint128::zero(),
                    }),
            ];

            // Get fee amount
            let fee_info = query_fee_info(
                &deps.querier,
                ASTROPORT_FACTORY.load(deps.storage)?,
                pool.pair_type.clone(),
            )?;
            let fee = fee_info.total_fee_rate;

            // Calculate amount of tokens to swap
            let (offer_asset, return_asset) = calc_xyk_balancing_swap(
                assets_slice,
                pool_reserves[0].amount,
                pool_reserves[1].amount,
                fee,
            )?;
            // Update balances for liquidity provision
            assets.add(&return_asset)?;
            assets.deduct(&offer_asset)?;

            let mut response = Response::new();
            // Create message to swap some of the asset to the other
            if offer_asset.amount > Uint128::zero() {
                let swap_res = pool.swap(
                    deps.as_ref(),
                    &env,
                    offer_asset,
                    return_asset.info.clone(),
                    Uint128::one(),
                )?;
                response = swap_res;
            }

            // Create message to provide liquidity
            let provide_msg = CallbackMsg::ProvideLiquidity {
                assets: assets.clone(),
                min_out,
                pool: pool.clone(),
            }
            .into_cosmos_msg(&env)?;
            response = response.add_message(provide_msg);

            // Callback to return LP tokens
            let callback_msg = CallbackMsg::ReturnLpTokens {
                pool,
                balance_before: lp_token_balance,
                recipient,
            }
            .into_cosmos_msg(&env)?;

            let event =
                Event::new("apollo/astroport-liquidity-helper/execute_balancing_provide_liquidity")
                    .add_attribute("action", "xyk_provide_liquidity")
                    .add_attribute("assets", assets.to_string())
                    .add_attribute("min_out", min_out);

            Ok(merge_responses(vec![receive_res, response])
                .add_message(callback_msg)
                .add_event(event))
        }
        PairType::Stable {} => {
            // For stable pools we are allowed to provide liquidity in any ratio,
            // so we simply provide liquidity with all passed assets.
            let provide_liquidity_res =
                pool.provide_liquidity(deps.as_ref(), &env, assets.clone(), min_out)?;

            // Callback to return LP tokens
            let callback_msg = CallbackMsg::ReturnLpTokens {
                pool,
                balance_before: lp_token_balance,
                recipient,
            }
            .into_cosmos_msg(&env)?;

            let event =
                Event::new("apollo/astroport-liquidity-helper/execute_balancing_provide_liquidity")
                    .add_attribute("action", "stable_provide_liquidity")
                    .add_attribute("assets", assets.to_string())
                    .add_attribute("min_out", min_out);

            Ok(merge_responses(vec![receive_res, provide_liquidity_res])
                .add_message(callback_msg)
                .add_event(event))
        }
        PairType::Custom(_) => Err(ContractError::CustomPairType {}),
    }
}

/// CallbackMsg handler to provide liquidity with the given assets. This needs
/// to be a callback, rather than doing in the first ExecuteMsg, because
/// pool.provide_liquidity does a simulation with current reserves, and we do a
/// swap in the top level ExecuteMsg, which means the reserves would be wrong in
/// the provide liquidity simulation.
pub fn execute_callback_provide_liquidity(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    assets: AssetList,
    min_out: Uint128,
    pool: AstroportPool,
) -> Result<Response, ContractError> {
    let res = pool.provide_liquidity(deps.as_ref(), &env, assets.clone(), min_out)?;

    let event = Event::new("apollo/astroport-liquidity-helper/execute_callback_provide_liquidity")
        .add_attribute("assets", assets.to_string());

    Ok(res.add_event(event))
}

pub fn execute_callback_return_lp_tokens(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    pool: AstroportPool,
    balance_before: Uint128,
    recipient: Addr,
) -> Result<Response, ContractError> {
    let lp_token = pool.lp_token();
    let lp_token_balance = lp_token.query_balance(&deps.querier, env.contract.address)?;

    let return_amount = lp_token_balance.checked_sub(balance_before)?;
    let return_asset = Asset::new(lp_token, return_amount);
    let msg = return_asset.transfer_msg(&recipient)?;

    let event = Event::new("apollo/astroport-liquidity-helper/execute_callback_return_lp_tokens")
        .add_attribute("return_asset", return_asset.to_string())
        .add_attribute("recipient", recipient);

    Ok(Response::new().add_message(msg).add_event(event))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::AstroportFactory {} => to_binary(&ASTROPORT_FACTORY.load(deps.storage)?),
    }
}

#[cfg(test)]
mod tests {}
