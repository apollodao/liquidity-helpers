use apollo_cw_asset::{Asset, AssetInfo, AssetList};
use astroport_liquidity_helper::math::calc_xyk_balancing_swap;
use astroport_liquidity_helper::msg::InstantiateMsg;
use cosmwasm_std::{assert_approx_eq, coin, to_binary, Addr, Coin, Decimal, Uint128};
use cw20::{AllowanceResponse, BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
use cw_dex::astroport::astroport::asset::{Asset as AstroAsset, AssetInfo as AstroAssetInfo};
use cw_dex::astroport::astroport::factory::{
    FeeInfoResponse, PairType, QueryMsg as FactoryQueryMsg,
};
use cw_dex::astroport::astroport::pair::{
    ExecuteMsg as PairExecuteMsg, PoolResponse, QueryMsg as PairQueryMsg, SimulationResponse,
    StablePoolParams,
};
use cw_dex::astroport::{astroport, AstroportPool};
use cw_it::astroport::utils::{
    create_astroport_pair, get_local_contracts, instantiate_astroport, AstroportContracts,
};
use cw_it::helpers::upload_wasm_files;
use cw_it::osmosis_test_tube::osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest;
use cw_it::traits::CwItRunner;
use cw_it::TestRunner;
use liquidity_helper::LiquidityHelper;
use test_case::test_matrix;

use cw_it::osmosis_test_tube::cosmrs::proto::cosmwasm::wasm::v1::MsgExecuteContractResponse;
use cw_it::osmosis_test_tube::{
    Account, Bank, Module, OsmosisTestApp, Runner, SigningAccount, Wasm,
};
use std::collections::HashMap;
use std::str::FromStr;

pub const ASTROPORT_LIQUIDITY_HELPER_WASM_FILE: &str =
    "../../target/wasm32-unknown-unknown/release/astroport_liquidity_helper.wasm";

/// Runs tests against the Osmosis bindings.
/// This works since there are no big differences between Terra and Osmosis.
pub fn setup(runner: &TestRunner) -> (Vec<SigningAccount>, HashMap<String, u64>) {
    let accs = runner
        .init_accounts(
            &[
                coin(1_000_000_000_000_000, "uluna"),
                coin(1_000_000_000_000_000, "uosmo"),
            ],
            2,
        )
        .unwrap();

    // Upload astroport contracts
    let contracts = get_local_contracts(runner, &Some("tests/astroport-artifacts"), false, &None);
    let astroport_code_ids = upload_wasm_files(runner, &accs[0], contracts).unwrap();

    (accs, astroport_code_ids)
}

/// Instantiates the liquidity helper contract
pub fn setup_astroport_liquidity_provider_tests<'a, R: Runner<'a>>(
    app: &'a R,
    accs: &[SigningAccount],
    astroport_contracts: &AstroportContracts,
) -> LiquidityHelper
where
{
    let wasm = Wasm::new(app);
    let admin = &accs[0];

    println!("Uploading liquidity helper wasm");

    // Load compiled wasm bytecode
    let astroport_liquidity_helper_wasm_byte_code =
        std::fs::read(ASTROPORT_LIQUIDITY_HELPER_WASM_FILE).unwrap();
    let astroport_liquidity_helper_code_id = wasm
        .store_code(&astroport_liquidity_helper_wasm_byte_code, None, admin)
        .unwrap()
        .data
        .code_id;

    println!("Instantiating liquidity helper contract");

    // Instantiate the contract
    let astroport_liquidity_helper = wasm
        .instantiate(
            astroport_liquidity_helper_code_id,
            &InstantiateMsg {
                astroport_factory: astroport_contracts.factory.address.clone(),
            },
            Some(&admin.address()), // contract admin used for migration
            Some("Astroport Liquidity Helper"), // contract label
            &[],                    // funds
            admin,                  // signer
        )
        .unwrap()
        .data
        .address;

    LiquidityHelper::new(Addr::unchecked(astroport_liquidity_helper))
}

#[test]
pub fn test_calc_xyk_balancing_swap() {
    let app = OsmosisTestApp::default();
    let runner = TestRunner::OsmosisTestApp(&app);
    let (accs, astroport_code_ids) = setup(&runner);
    let wasm = Wasm::new(&runner);
    let admin = &accs[0];

    // Instantiate Astroport contracts
    let astroport_contracts = instantiate_astroport(&runner, admin, &astroport_code_ids);

    let astro_token = astroport_contracts.astro_token.address.clone();

    // Create 1:1 XYK pool
    let asset_infos: [AstroAssetInfo; 2] = [
        AstroAssetInfo::NativeToken {
            denom: "uluna".into(),
        },
        AstroAssetInfo::Token {
            contract_addr: Addr::unchecked(&astro_token),
        },
    ];
    let (uluna_astro_pair_addr, _) = create_astroport_pair(
        &runner,
        &astroport_contracts.factory.address,
        PairType::Xyk {},
        asset_infos.clone(),
        None,
        admin,
        None,
    );

    // Increase allowance of astro token for Pair contract
    let increase_allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: uluna_astro_pair_addr.clone(),
        amount: Uint128::from(3_000_000_000_000u128),
        expires: None,
    };
    let _res = wasm
        .execute(&astro_token, &increase_allowance_msg, &[], admin)
        .unwrap();

    // Provide liquidity
    let provide_liq_msg = PairExecuteMsg::ProvideLiquidity {
        assets: [
            AstroAsset {
                amount: Uint128::from(1_000_000_000_000u128),
                info: AstroAssetInfo::NativeToken {
                    denom: "uluna".into(),
                },
            },
            AstroAsset {
                amount: Uint128::from(3_000_000_000_000u128),
                info: AstroAssetInfo::Token {
                    contract_addr: Addr::unchecked(&astro_token),
                },
            },
        ]
        .to_vec(),
        slippage_tolerance: Some(Decimal::from_str("0.02").unwrap()),
        auto_stake: Some(false),
        receiver: None,
    };
    let _res = wasm.execute(
        &uluna_astro_pair_addr,
        &provide_liq_msg,
        &[Coin {
            amount: Uint128::from(1_000_000_000_000u128),
            denom: "uluna".into(),
        }],
        admin,
    );

    // Query fee info
    let res: FeeInfoResponse = wasm
        .query(
            &astroport_contracts.factory.address,
            &FactoryQueryMsg::FeeInfo {
                pair_type: PairType::Xyk {},
            },
        )
        .unwrap();
    let total_fee_rate = Decimal::from_ratio(res.total_fee_bps, 10000u16);

    // Calculate balancing swap
    let assets = [
        Asset {
            amount: Uint128::from(1_000_000_000_000u128),
            info: AssetInfo::native("uluna".to_string()),
        },
        Asset {
            amount: Uint128::from(1_000_000_000_000u128),
            info: AssetInfo::Cw20(Addr::unchecked(&astro_token)),
        },
    ];
    let reserves = [
        Uint128::from(1_000_000_000_000u128),
        Uint128::from(3_000_000_000_000u128),
    ];

    let (offer_asset, return_asset) =
        calc_xyk_balancing_swap(assets, reserves, total_fee_rate).unwrap();

    // Simulate swap
    let simulation_result: SimulationResponse = wasm
        .query(
            &uluna_astro_pair_addr,
            &PairQueryMsg::Simulation {
                offer_asset: AstroAsset {
                    amount: offer_asset.amount,
                    info: asset_infos[0].clone(),
                },
                ask_asset_info: Some(return_asset.info.into()),
            },
        )
        .unwrap();

    // Check if the simulation result is correct
    assert_eq!(simulation_result.return_amount, return_asset.amount);
}

// Test 1: 1:1 ratio, double amount of asset 2
#[test_matrix(
    [[Uint128::from(1_000_000u128), Uint128::from(2_000_000u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string())],
    true
)]
// Test 2: 1:5 ratio, double amount of asset 2
#[test_matrix(
    [[Uint128::from(1_000_000u128), Uint128::from(2_000_000u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(5_000_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string())],
    true
)]
// Test 3: 1:2.9 pool ratio, 1:1 ratio of assets, but a lot of assets compared to pool (high
// slipage)
#[test_matrix(
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(2_900_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string())],
    true
)]
// Test 4: 1:2 pool ratio, 0:1 ratio of assets
#[test_matrix(
    [[Uint128::from(0u128), Uint128::from(1_000_000_000_000u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(2_000_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string())],
    true
)]
// Test 5: 1:1 pool ratio, 1:1 ratio of assets
#[test_matrix(
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string())],
    true
)]
// Test 6: 1:1 pool ratio, 1:0 ratio of assets
#[test_matrix(
    [[Uint128::from(1_000_000_000u128), Uint128::from(0u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string())],
    true
)]
#[test_case(
    [Uint128::from(0u128), Uint128::from(3564u128)],
    [Uint128::from(3450765745u128), Uint128::from(12282531965699u128)],
    PairType::Xyk {},
    false;
    "Test 7: Xyk amount of asset less than one microunit of other asset"
)]
// Test 7: Amount of asset would be less than one microunit of other asset if it were xyk
#[test_matrix(
    [[Uint128::from(0u128), Uint128::from(3564u128)]],
    [[Uint128::from(3450765745u128), Uint128::from(12282531965699u128)]],
    [PairType::Stable {  }, PairType::Custom("concentrated".to_string())],
    true
)]
#[test_case(
    [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
    [Uint128::from(0u128), Uint128::from(0u128)],
    PairType::Xyk {},
    true
    => panics "No liquidity in pool";
    "Test 8 Xyk: 0:0 pool ratio, should fail with correct error"
)]
// Test 8: empty pool. Should work for stable and concentrated pools, but not for xyk pools.
#[test_matrix(
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [[Uint128::from(0u128), Uint128::from(0u128)]],
    [PairType::Stable {  }, PairType::Custom("concentrated".to_string())],
    true
)]
/// Tests the BalancingProvideLiquidity message
pub fn test_balancing_provide_liquidity(
    asset_amounts: [Uint128; 2],
    reserves: [Uint128; 2],
    pair_type: PairType,
    should_provide: bool,
) {
    let app = OsmosisTestApp::default();
    let runner = TestRunner::OsmosisTestApp(&app);
    let (accs, astroport_code_ids) = &setup(&runner);
    let admin = &accs[0];
    let wasm = Wasm::new(&runner);

    // Instantiate Astroport contracts
    let astroport_contracts = instantiate_astroport(&runner, admin, astroport_code_ids);

    // Update native coin registry with uluna precision
    wasm.execute(
        &astroport_contracts.coin_registry.address,
        &astroport::native_coin_registry::ExecuteMsg::Add {
            native_coins: vec![("uluna".to_string(), 6)],
        },
        &[],
        admin,
    )
    .unwrap();

    let liquidity_helper =
        setup_astroport_liquidity_provider_tests(&runner, accs, &astroport_contracts);
    let astro_token = astroport_contracts.astro_token.address.clone();

    // Create pool
    let asset_infos: [AstroAssetInfo; 2] = [
        AstroAssetInfo::NativeToken {
            denom: "uluna".into(),
        },
        AstroAssetInfo::Token {
            contract_addr: Addr::unchecked(&astro_token),
        },
    ];
    let init_params = match &pair_type {
        PairType::Stable {} => Some(
            to_binary(&StablePoolParams {
                amp: 10u64,
                owner: None,
            })
            .unwrap(),
        ),
        PairType::Custom(t) => match t.as_str() {
            "concentrated" => Some(to_binary(&cw_dex_test_helpers::common_pcl_params()).unwrap()),
            _ => None,
        },
        _ => None,
    };
    let (uluna_astro_pair_addr, uluna_astro_lp_token) = create_astroport_pair(
        &runner,
        &astroport_contracts.factory.address,
        pair_type.clone(),
        asset_infos,
        init_params,
        admin,
        None,
    );
    let pool = AstroportPool {
        lp_token_addr: Addr::unchecked(uluna_astro_lp_token),
        pair_addr: Addr::unchecked(uluna_astro_pair_addr.clone()),
        pair_type,
        pool_assets: vec![
            AssetInfo::native("uluna".to_string()),
            AssetInfo::cw20(Addr::unchecked(&astro_token)),
        ],
        liquidity_manager: Addr::unchecked(astroport_contracts.liquidity_manager.address),
    };

    // Increase allowance of astro token for Pair contract
    let increase_allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: uluna_astro_pair_addr.clone(),
        amount: reserves[1],
        expires: None,
    };
    let _res = wasm
        .execute(&astro_token, &increase_allowance_msg, &[], admin)
        .unwrap();

    // Query allowance
    let allowance_res: AllowanceResponse = wasm
        .query(
            &astro_token,
            &Cw20QueryMsg::Allowance {
                owner: admin.address(),
                spender: uluna_astro_pair_addr.clone(),
            },
        )
        .unwrap();
    assert_eq!(allowance_res.allowance, reserves[1]);

    // Add initial pool liquidity
    let provide_liq_msg = PairExecuteMsg::ProvideLiquidity {
        assets: [
            AstroAsset {
                amount: reserves[0],
                info: AstroAssetInfo::NativeToken {
                    denom: "uluna".into(),
                },
            },
            AstroAsset {
                amount: reserves[1],
                info: AstroAssetInfo::Token {
                    contract_addr: Addr::unchecked(&astro_token),
                },
            },
        ]
        .to_vec(),
        slippage_tolerance: Some(Decimal::from_str("0.02").unwrap()),
        auto_stake: Some(false),
        receiver: None,
    };
    let _res = wasm.execute(
        &uluna_astro_pair_addr,
        &provide_liq_msg,
        &[Coin {
            amount: reserves[0],
            denom: "uluna".into(),
        }],
        admin,
    );

    // Check pool liquidity after adding
    let initial_pool_liquidity: PoolResponse = wasm
        .query(&uluna_astro_pair_addr, &PairQueryMsg::Pool {})
        .unwrap();
    println!("initial_pool_liquidity: {initial_pool_liquidity:?}");
    if let AstroAssetInfo::NativeToken { denom: _ } = &initial_pool_liquidity.assets[0].info {
        assert_eq!(initial_pool_liquidity.assets[0].amount, reserves[0]);
        assert_eq!(initial_pool_liquidity.assets[1].amount, reserves[1]);
    } else {
        assert_eq!(initial_pool_liquidity.assets[0].amount, reserves[1]);
        assert_eq!(initial_pool_liquidity.assets[1].amount, reserves[0]);
    }

    // Check asset balances before balancing provide liquidity
    let uluna_balance_before = query_token_balance(&runner, &admin.address(), "uluna");
    let astro_balance_before = query_cw20_balance(&runner, admin.address(), &astro_token);

    // Balancing Provide liquidity
    println!("Balancing provide liquidity");
    let mut assets: AssetList = vec![Coin::new(asset_amounts[0].u128(), "uluna")].into();
    assets
        .add(&Asset::new(
            AssetInfo::Cw20(Addr::unchecked(&astro_token)),
            asset_amounts[1],
        ))
        .unwrap();
    let msgs = liquidity_helper
        .balancing_provide_liquidity(assets, Uint128::zero(), to_binary(&pool).unwrap(), None)
        .unwrap();
    let _res = runner
        .execute_cosmos_msgs::<MsgExecuteContractResponse>(&msgs, admin)
        .unwrap();

    // Check pool liquidity after adding
    let pool_liquidity = wasm
        .query::<_, PoolResponse>(&uluna_astro_pair_addr, &PairQueryMsg::Pool {})
        .unwrap()
        .assets;
    // Check asset balances after balancing provide liquidity.
    let uluna_balance_after = query_token_balance(&runner, &admin.address(), "uluna");
    let astro_balance_after = query_cw20_balance(&runner, admin.address(), &astro_token);
    if should_provide {
        // Astroport liquidity manager rounds down the amount of tokens sent to the pool
        // by one unit.
        assert_approx_eq!(
            pool_liquidity[0].amount,
            reserves[0] + asset_amounts[0],
            "0.00000001"
        );
        assert_approx_eq!(
            pool_liquidity[1].amount,
            reserves[1] + asset_amounts[1],
            "0.00000001"
        );

        // Should have used all assets
        assert_eq!(uluna_balance_before - uluna_balance_after, asset_amounts[0]);
        assert_eq!(astro_balance_before - astro_balance_after, asset_amounts[1]);
    } else {
        assert_eq!(pool_liquidity[0].amount, reserves[0]);
        assert_eq!(pool_liquidity[1].amount, reserves[1]);

        // Should have returned the assets if providing liquidity was not possible.
        assert_eq!(uluna_balance_before - uluna_balance_after, Uint128::zero());
        assert_eq!(astro_balance_before - astro_balance_after, Uint128::zero());
    }
}

fn query_token_balance<'a, R>(runner: &'a R, address: &str, denom: &str) -> Uint128
where
    R: Runner<'a>,
{
    let bank = Bank::new(runner);
    let balance = bank
        .query_balance(&QueryBalanceRequest {
            address: address.to_string(),
            denom: denom.to_string(),
        })
        .unwrap()
        .balance
        .unwrap_or_default()
        .amount;
    Uint128::from_str(&balance).unwrap()
}

fn query_cw20_balance<'a, R, S>(runner: &'a R, address: S, contract_addr: &str) -> Uint128
where
    R: Runner<'a>,
    S: Into<String>,
{
    let wasm = Wasm::new(runner);
    wasm.query::<_, BalanceResponse>(
        contract_addr,
        &Cw20QueryMsg::Balance {
            address: address.into(),
        },
    )
    .unwrap()
    .balance
}

#[test_matrix(
    3,
    4,
    [PairType::Xyk {}, PairType::Stable {}]
)]
fn multiplication_tests(x: i8, y: i8, pair_type: PairType) {
    // let actual = (x * y).abs();
    println!("x: {}, y: {}", x, y);
    println!("pair_type: {:?}", pair_type);

    // assert_eq!(8, actual)
}
