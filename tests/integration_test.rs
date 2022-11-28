use cosmwasm_std::{to_binary, Addr, Coin, Uint128};
use cw_dex::osmosis::OsmosisPool;
use cw_it::app::App as RpcRunner;
use cw_it::Cli;
use osmosis_liquidity_helper::{helpers::LiquidityHelper, msg::InstantiateMsg};
use osmosis_testing::{
    cosmrs::proto::cosmwasm::wasm::v1::MsgExecuteContractResponse, Account, Gamm, Module,
    OsmosisTestApp, Runner, SigningAccount, Wasm,
};

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

#[test]
/// Runs all tests against LocalOsmosis
pub fn test_with_localosmosis() {
    let docker: Cli = Cli::default();
    let app = RpcRunner::new(TEST_CONFIG_PATH, &docker);

    let accs = app
        .test_config
        .import_all_accounts()
        .into_values()
        .collect::<Vec<_>>();

    test_balancing_provide_liquidity(&app, accs);
}

#[test]
/// Runs all tests against the Osmosis bindings
pub fn test_with_osmosis_bindings() {
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

    test_balancing_provide_liquidity(&app, accs);
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

    // let liquidity_helper = LiquidityHelperBase(contract_addr).check(api).unwrap(); // TODO this errors with "human address too long". Why?
    let liquidity_helper = LiquidityHelper::new(Addr::unchecked(contract_addr));

    liquidity_helper
}

/// Tests the BalancingProvideLiquidity message
pub fn test_balancing_provide_liquidity<R>(app: &R, accs: Vec<SigningAccount>)
where
    R: for<'a> Runner<'a>,
{
    let liquidity_helper = setup_osmosis_liquidity_provider_tests(app, &accs);
    let gamm = Gamm::new(app);

    // Create 1:1 pool
    let pool_liquidity = vec![Coin::new(1_000_000, "uatom"), Coin::new(1_000_000, "uosmo")];
    let pool_id = gamm
        .create_basic_pool(&pool_liquidity, &accs[0])
        .unwrap()
        .data
        .pool_id;
    let pool = OsmosisPool::new(pool_id);

    // Balancing Provide liquidity
    println!("Balancing provide liquidity");
    let coins = vec![Coin::new(100_000, "uatom"), Coin::new(100_000, "uosmo")];
    let msgs = liquidity_helper
        .balancing_provide_liquidity(
            coins.clone().into(),
            Uint128::one(),
            to_binary(&pool).unwrap(),
            None,
        )
        .unwrap();
    let _res = app
        .execute_cosmos_msgs::<MsgExecuteContractResponse>(&msgs, &accs[1])
        .unwrap();

    // Check pool liquidity after adding
    let initial_pool_liquidity = vec![Coin::new(1_000_000, "uatom"), Coin::new(1_000_000, "uosmo")];
    let pool_liquidity = gamm.query_pool_reserves(pool_id).unwrap();
    assert_eq!(
        pool_liquidity,
        merge_coins(&[&initial_pool_liquidity, &coins])
    );
}