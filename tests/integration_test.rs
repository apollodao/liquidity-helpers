use std::vec;

use cosmwasm_std::{to_binary, Addr, Coin, StdError, Uint128};
use cw_asset::{Asset, AssetInfo, AssetList};
use cw_dex::osmosis::OsmosisPool;
use cw_it::app::App as RpcRunner;
use cw_it::Cli;
use osmosis_liquidity_helper::{helpers::LiquidityHelper, msg::InstantiateMsg};
use osmosis_testing::{
    cosmrs::proto::cosmwasm::wasm::v1::MsgExecuteContractResponse, Account, Gamm, Module,
    OsmosisTestApp, Runner, RunnerError, RunnerResult, SigningAccount, Wasm,
};

use test_case::test_case;

pub const WASM_FILE: &str = "artifacts/osmosis_liquidity_helper.wasm";
const TEST_CONFIG_PATH: &str = "tests/configs/osmosis.yaml";

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

// NOTE: I don't think this is the error we want
const CW20_ERROR: &str = "failed to execute message; message index: 0: contract: not found";

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

#[test_case(assets_native("uatom", Some("uosmo"), 100_000).into(), assets_native("uatom", Some("uosmo"), 1_000_000), ONE ; "LocalOsmosis: Balanced native assets")]
#[test_case(assets_native("uatom", None, 100_000).into(), assets_native("uatom", Some("uosmo"), 1_000_000), ONE ; "LocalOsmosis: Single native asset")]
#[test_case(assets_cw20(100_000), assets_native("uatom", Some("uosmo"), 1_000_000), ONE ; "LocalOsmosis: Non-native assets")]
/// Runs all tests against LocalOsmosis
pub fn test_with_localosmosis(
    assets: AssetList,
    pool_liquidity: Vec<Coin>,
    min_out: Uint128,
) -> RunnerResult<()> {
    let docker: Cli = Cli::default();
    let app = RpcRunner::new(TEST_CONFIG_PATH, &docker);

    let accs = app
        .test_config
        .import_all_accounts()
        .into_values()
        .collect::<Vec<_>>();

    test_balancing_provide_liquidity(&app, accs, assets.into(), pool_liquidity, min_out)
}

// TODO add more tests
#[test_case(assets_native("uatom", Some("uosmo"), 100_000).into(), assets_native("uatom", Some("uosmo"), 1_000_000), ONE ; "Bindings: Balanced native assets")]
#[test_case(assets_native("uatom", None, 100_000).into(), assets_native("uatom", Some("uosmo"), 1_000_000), ONE ; "Bindings: Single native asset")]
#[test_case(assets_native("uosmo", None, 100_000).into(), assets_native("uatom", Some("uosmo"), 1_000_000), ONE ; "Bindings: Single native asset 2")]
#[test_case(assets_cw20(100_000), assets_native("uatom", Some("uosmo"), 1_000_000), ONE => Err(RunnerError::ExecuteError { msg: CW20_ERROR.to_string() }); "Bindings: Non-native assets")]
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
    pool_liquidity: Vec<Coin>,
    min_out: Uint128,
) -> RunnerResult<()>
where
    R: for<'a> Runner<'a>,
{
    let liquidity_helper = setup_osmosis_liquidity_provider_tests(app, &accs);
    let gamm = Gamm::new(app);

    // Create 1:1 pool
    let pool_id = gamm
        .create_basic_pool(&pool_liquidity, &accs[0])
        .unwrap()
        .data
        .pool_id;
    let pool = OsmosisPool::new(pool_id);

    // Balancing Provide liquidity
    let msgs = liquidity_helper.balancing_provide_liquidity(
        assets.clone(),
        min_out,
        to_binary(&pool)?,
        None,
    )?;

    let _res = app.execute_cosmos_msgs::<MsgExecuteContractResponse>(&msgs, &accs[1])?;

    // Convert assets to native coins
    let mut coins: Vec<Coin> = vec![];
    for a in assets.into_iter() {
        coins.push(a.try_into()?)
    }

    // Check pool liquidity after adding
    let initial_pool_liquidity = vec![Coin::new(1_000_000, "uatom"), Coin::new(1_000_000, "uosmo")];
    let pool_liquidity = gamm.query_pool_reserves(pool_id).unwrap();
    assert_eq!(
        pool_liquidity,
        merge_coins(&[&initial_pool_liquidity, &coins])
    );
    Ok(())
}
