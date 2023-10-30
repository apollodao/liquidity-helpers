use apollo_cw_asset::{Asset, AssetList};
use apollo_utils::assets::receive_assets;
use apollo_utils::responses::merge_responses;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env, Event,
    MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw_dex::astroport::astroport::factory::PairType;
use cw_dex::astroport::astroport::querier::query_fee_info;

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
    let recipient = recipient.map_or(Ok(info.sender.clone()), |x| deps.api.addr_validate(&x))?;

    // Check lp token balance before, to pass into callback
    let lp_token_balance = pool
        .lp_token()
        .query_balance(&deps.querier, env.contract.address.to_string())?;

    // For XYK pools we need to swap some amount of one asset into the other before
    // we provide liquidity. For other types we can just provide liquidity
    // directly.
    let swap_res = match &pool.pair_type {
        PairType::Xyk {} => {
            let pool_res = pool.query_pool_info(&deps.querier)?;

            let pool_reserves: [Asset; 2] = [
                Asset::from(pool_res.assets[0].clone()),
                Asset::from(pool_res.assets[1].clone()),
            ];
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
                [pool_reserves[0].amount, pool_reserves[1].amount],
                fee,
            )?;
            // Update balances for liquidity provision
            assets.add(&return_asset)?;
            assets.deduct(&offer_asset)?;

            // If either of the assets are still zero after the swap, we can't
            // provide liquidity. This can happen if the amount of tokens to swap
            // is so small that the returned amount of the other asset would be zero.
            if pool.pool_assets.iter().any(|x| {
                assets
                    .find(x)
                    .map_or_else(Uint128::zero, |y| y.amount)
                    .is_zero()
            }) {
                if min_out.is_zero() {
                    // If min_out is zero, we can just return the received native
                    // assets. We don't need to return any Cw20 assets, because
                    // we did not execute the transferFrom on them.
                    let return_msg = CosmosMsg::Bank(BankMsg::Send {
                        to_address: info.sender.to_string(),
                        amount: info.funds,
                    });
                    let event = Event::new(
                        "apollo/astroport-liquidity-helper/execute_balancing_provide_liquidity",
                    )
                    .add_attribute("action", "No liquidity provided. Zero amount of asset")
                    .add_attribute("assets", assets.to_string())
                    .add_attribute("min_out", min_out);
                    return Ok(Response::new().add_message(return_msg).add_event(event));
                } else {
                    // If min_out is not zero, we need to return an error
                    return Err(ContractError::MinOutNotReceived {
                        min_out,
                        received: Uint128::zero(),
                    });
                }
            }

            let mut swap_res = Response::new();
            // Create message to swap some of the asset to the other
            if offer_asset.amount > Uint128::zero() && return_asset.amount > Uint128::zero() {
                swap_res = pool.swap(
                    deps.as_ref(),
                    &env,
                    offer_asset,
                    return_asset.info.clone(),
                    Uint128::one(),
                )?;
            }

            swap_res
        }
        PairType::Custom(t) => match t.as_str() {
            "concentrated" => Response::new(),
            _ => return Err(ContractError::UnsupportedPairType {}),
        },
        _ => Response::new(),
    };

    // For stableswap and concentrated liquidity pools we are allowed to provide
    // liquidity in any ratio, so we simply provide liquidity with all passed
    // assets.
    let provide_liquidity_res =
        pool.provide_liquidity(deps.as_ref(), &env, assets.clone(), min_out)?;

    // Callback to return LP tokens
    let callback_msg = CallbackMsg::ReturnLpTokens {
        pool,
        balance_before: lp_token_balance,
        recipient,
    }
    .into_cosmos_msg(&env)?;

    let event: Event =
        Event::new("apollo/astroport-liquidity-helper/execute_balancing_provide_liquidity")
            .add_attribute("assets", assets.to_string())
            .add_attribute("min_out", min_out);

    Ok(
        merge_responses(vec![receive_res, swap_res, provide_liquidity_res])
            .add_message(callback_msg)
            .add_event(event),
    )
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
