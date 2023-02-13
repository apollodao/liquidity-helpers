use std::str::FromStr;
use std::vec;

use cosmwasm_std::{to_binary, Addr, Coin, Uint128};
use cw_asset::{Asset, AssetList};
use cw_dex::osmosis::OsmosisPool;
use liquidity_helper::LiquidityHelper;
use osmosis_liquidity_helper::msg::InstantiateMsg;
use osmosis_testing::cosmrs::proto::cosmos::bank::v1beta1::QueryBalanceRequest;
use osmosis_testing::Bank;
use osmosis_testing::{
    cosmrs::proto::cosmwasm::wasm::v1::MsgExecuteContractResponse, Account, Gamm, Module,
    OsmosisTestApp, Runner, RunnerResult, SigningAccount, Wasm,
};

use test_case::test_case;

pub const WASM_FILE: &str =
    "../../target/wasm32-unknown-unknown/release/osmosis_liquidity_helper.wasm";

/// Merges a list of list of coins into a single list of coins, adding amounts
/// when denoms are the same.
fn merge_coins(coins: &[&[Coin]]) -> Vec<Coin> {
    let mut merged: Vec<Coin> = vec![];
    for coin_list in coins {
        for coin in *coin_list {
            let mut found = false;
            merged.iter_mut().for_each(|c| {
                if c.denom == coin.denom {
                    c.amount += coin.amount;
                    found = true;
                }
            });
            if !found {
                merged.push(coin.clone());
            }
        }
    }
    merged
}

const ONE: Uint128 = Uint128::one();

fn assets_native(first: &str, second: Option<&str>, amount: u128) -> Vec<Coin> {
    if let Some(denom) = second {
        vec![Coin::new(amount, first), Coin::new(amount, denom)]
    } else {
        vec![Coin::new(amount, first)]
    }
}

fn assets_cw20(amount: u128) -> AssetList {
    vec![
        Asset::cw20(
            Addr::unchecked("osmo14gs9zqh8m49yy9kscjqu9h72exyf295afg6kgk"),
            amount,
        ),
        Asset::cw20(
            Addr::unchecked("osmo10qfrpash5g2vk3hppvu45x0g860czur8ff5yx0"),
            amount,
        ),
    ]
    .into()
}

#[test_case(assets_native("uatom", Some("uosmo"), 100_000).into(), assets_native("uatom", Some("uosmo"), 1_000_000), ONE ; "Bindings: Balanced native assets")]
#[test_case(assets_native("uatom", None, 100_000).into(), assets_native("uatom", Some("uosmo"), 1_000_000), ONE ; "Bindings: Single native asset")]
#[test_case(assets_native("uosmo", None, 100_000).into(), assets_native("uatom", Some("uosmo"), 1_000_000), ONE ; "Bindings: Single native asset 2")]
#[test_case(assets_cw20(100_000), assets_native("uatom", Some("uosmo"), 1_000_000), ONE => matches Err(_); "Bindings: Non-native assets")]
#[test_case(vec![Coin::new(3_000, "uatom"), Coin::new(1_000, "uosmo")].into(), assets_native("uatom",Some("uosmo"),1_000_000), ONE; "Bindings: Unbalanced assets in balanced pool")]
#[test_case(vec![Coin::new(100, "uatom"), Coin::new(4_000, "uosmo")].into(), assets_native("uatom",Some("uosmo"),1_000_000), ONE; "Bindings: Unbalanced assets in balanced pool 2")]
#[test_case(vec![Coin::new(4_800_000, "uatom"), Coin::new(2_000_000, "uosmo")].into(), assets_native("uatom",Some("uosmo"),1_000_000), ONE; "Bindings: Unbalanced assets in balanced pool, high slippage")]
#[test_case(vec![Coin::new(1_800_000, "uatom"), Coin::new(2_000_000, "uosmo")].into(), vec![Coin::new(1_000_000, "uatom"),Coin::new(3_000_000,"uosmo")], ONE; "Bindings: Unbalanced assets in unbalanced pool, high slippage")]
/// Runs all tests against the Osmosis bindings
pub fn test_with_osmosis_bindings(
    assets: AssetList,
    pool_liquidity: Vec<Coin>,
    min_out: Uint128,
) -> RunnerResult<()> {
    let app = OsmosisTestApp::default();

    let accs = app
        .init_accounts(
            &[
                Coin::new(1_000_000_000_000, "uatom"),
                Coin::new(1_000_000_000_000, "uosmo"),
            ],
            2,
        )
        .unwrap();

    test_balancing_provide_liquidity(&app, accs, assets.into(), pool_liquidity, min_out)
}

/// Instantiates the liquidity helper contract
pub fn setup_osmosis_liquidity_provider_tests<R>(
    app: &R,
    accs: &[SigningAccount],
) -> LiquidityHelper
where
    R: for<'a> Runner<'a>,
{
    let wasm = Wasm::new(app);
    let admin = &accs[0];

    // Load compiled wasm bytecode
    let wasm_byte_code = std::fs::read(WASM_FILE).unwrap();
    let code_id = wasm
        .store_code(&wasm_byte_code, None, admin)
        .unwrap()
        .data
        .code_id;

    // Instantiate the contract
    let contract_addr = wasm
        .instantiate(
            code_id,
            &InstantiateMsg {},
            Some(&admin.address()), // contract admin used for migration
            Some("Osmosis Liquidity Helper"), // contract label
            &[],                    // funds
            admin,                  // signer
        )
        .unwrap()
        .data
        .address;

    LiquidityHelper::new(Addr::unchecked(contract_addr))
}

/// Tests the BalancingProvideLiquidity message
pub fn test_balancing_provide_liquidity<R>(
    app: &R,
    accs: Vec<SigningAccount>,
    assets: AssetList,
    initial_pool_liquidity: Vec<Coin>,
    min_out: Uint128,
) -> RunnerResult<()>
where
    R: for<'a> Runner<'a>,
{
    let liquidity_helper = setup_osmosis_liquidity_provider_tests(app, &accs);
    let gamm = Gamm::new(app);
    let bank = Bank::new(app);

    // Create 1:1 pool
    let pool_id = gamm
        .create_basic_pool(&initial_pool_liquidity, &accs[0])
        .unwrap()
        .data
        .pool_id;
    let pool = OsmosisPool::unchecked(pool_id);

    // LP token supply before adding
    let total_shares = gamm.query_pool(pool_id).unwrap().total_shares.unwrap();
    let lp_token_supply_before = Uint128::from_str(&total_shares.amount).unwrap();
    let lp_token_denom = total_shares.denom.clone();

    // Check users LP token balance before
    let lp_token_balance_before = Uint128::from_str(
        &bank
            .query_balance(&QueryBalanceRequest {
                address: accs[1].address().to_string(),
                denom: lp_token_denom.clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount,
    )
    .unwrap();

    // Balancing Provide liquidity
    let msgs = liquidity_helper.balancing_provide_liquidity(
        assets.clone(),
        min_out,
        to_binary(&pool)?,
        None,
    )?;
    app.execute_cosmos_msgs::<MsgExecuteContractResponse>(&msgs, &accs[1])?;

    // Convert assets to native coins
    let mut coins: Vec<Coin> = vec![];
    for a in assets.into_iter() {
        coins.push(a.try_into()?)
    }

    // Check pool liquidity after adding
    let pool_liquidity = gamm.query_pool_reserves(pool_id).unwrap();
    assert_eq!(
        pool_liquidity,
        merge_coins(&[&initial_pool_liquidity, &coins])
    );

    // Make sure caller got all LP tokens
    let lp_token_supply_after = Uint128::from_str(
        &gamm
            .query_pool(pool_id)
            .unwrap()
            .total_shares
            .unwrap()
            .amount,
    )
    .unwrap();
    let lp_tokens_added = lp_token_supply_after - lp_token_supply_before;
    let lp_token_balance_after = Uint128::from_str(
        &bank
            .query_balance(&QueryBalanceRequest {
                address: accs[1].address().to_string(),
                denom: lp_token_denom.clone(),
            })
            .unwrap()
            .balance
            .unwrap()
            .amount,
    )
    .unwrap();
    assert_eq!(
        lp_token_balance_after,
        lp_token_balance_before + lp_tokens_added
    );

    Ok(())
}
