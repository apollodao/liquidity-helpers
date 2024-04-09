use apollo_cw_asset::{Asset, AssetInfo, AssetList};

use astroport_liquidity_helper::math::calc_xyk_balancing_swap;
use astroport_liquidity_helper::msg::InstantiateMsg;
use cosmwasm_std::{assert_approx_eq, coin, to_json_binary, Addr, Coin, Decimal, Uint128};
use cw20::{AllowanceResponse, BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
use cw_dex_astroport::astroport::asset::{Asset as AstroAsset, AssetInfo as AstroAssetInfo};
use cw_dex_astroport::astroport::factory::{ExecuteMsg as FactoryExecuteMsg, FeeInfoResponse, PairType};
use cw_dex_astroport::astroport::pair::{
    ExecuteMsg as PairExecuteMsg, PoolResponse, QueryMsg as PairQueryMsg, SimulationResponse, StablePoolParams
};
use cw_dex_astroport::astroport::pair_concentrated::ConcentratedPoolParams;
use cw_dex_astroport::{astroport, AstroportPool};
use cw_it::astroport::astroport::factory::{PairConfig, QueryMsg as FactoryQueryMsg};
use cw_it::astroport::astroport_v3::pair_xyk_sale_tax::{SaleTaxInitParams, TaxConfig};
use cw_it::astroport::utils::{
    create_astroport_pair, get_local_contracts, setup_astroport, AstroportContracts,
};
use cw_it::cw_multi_test::ContractWrapper;

use cw_it::multi_test::MultiTestRunner;
use cw_it::osmosis_test_tube::osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest;
use cw_it::traits::CwItRunner;
use cw_it::{Artifact, ContractType, OwnedTestRunner, TestRunner};
use liquidity_helper::LiquidityHelper;
use test_case::test_matrix;

use cw_it::osmosis_test_tube::cosmrs::proto::cosmwasm::wasm::v1::MsgExecuteContractResponse;
use cw_it::osmosis_test_tube::{Account, Bank, Module, Runner, SigningAccount, Wasm};

use std::str::FromStr;

pub const ASTROPORT_LIQUIDITY_HELPER_WASM_FILE: &str =
    "../../target/wasm32-unknown-unknown/release/astroport_liquidity_helper.wasm";

pub fn get_test_runner<'a>() -> OwnedTestRunner<'a> {
    match option_env!("TEST_RUNNER").unwrap_or("multi-test") {
        "multi-test" => {
            OwnedTestRunner::MultiTest(MultiTestRunner::new("osmo"))
        }
        #[cfg(feature = "osmosis-test-tube")]
        "osmosis-test-app" => OwnedTestRunner::OsmosisTestApp(OsmosisTestApp::new()),
        _ => panic!("Unsupported test runner type"),
    }
}

/// Instantiates the liquidity helper contract
pub fn setup_astroport_liquidity_provider_tests<'a>(
    app: &'a TestRunner<'a>,
    astroport_contracts: &AstroportContracts,
    admin: &SigningAccount,
) -> LiquidityHelper
where
{
    let wasm = Wasm::new(app);

    // Set uluna precision in native coin registry
    wasm.execute(
        &astroport_contracts.coin_registry.address,
        &astroport::native_coin_registry::ExecuteMsg::Add {
            native_coins: vec![("uluna".to_string(), 6)],
        },
        &[],
        admin,
    )
    .unwrap();

    // Upload astroport pair xyk sale tax contract
    let sale_tax_contract = match app {
        TestRunner::OsmosisTestApp(_) => ContractType::Artifact(Artifact::Local(
            ("tests/astroport-artifacts/astroport_pair_xyk_sale_tax.wasm").to_string(),
        )),
        TestRunner::MultiTest(_) => ContractType::MultiTestContract(Box::new(
            ContractWrapper::new_with_empty(
                astroport_pair_xyk_sale_tax::contract::execute,
                astroport_pair_xyk_sale_tax::contract::instantiate,
                astroport_pair_xyk_sale_tax::contract::query,
            )
            .with_reply(astroport_pair_xyk_sale_tax::contract::reply),
        )),
        _ => panic!("Unsupported runner"),
    };
    let sale_tax_code_id = app.store_code(sale_tax_contract, admin).unwrap();

    // Add XYK Sale Tax PairType to Astroport Factory
    wasm.execute(
        &astroport_contracts.factory.address,
        &FactoryExecuteMsg::UpdatePairConfig {
            config: PairConfig {
                code_id: sale_tax_code_id,
                is_disabled: false,
                is_generator_disabled: false,
                maker_fee_bps: 3333,
                total_fee_bps: 30,
                pair_type: PairType::Custom("astroport-pair-xyk-sale-tax".to_string()),
            },
        },
        &[],
        admin,
    )
    .unwrap();

    println!("Uploading liquidity helper wasm");

    // Load compiled wasm bytecode or multi-test contract depending on the runner
    let astroport_liquidity_helper_wasm_byte_code = match app {
        TestRunner::OsmosisTestApp(_) => ContractType::Artifact(Artifact::Local(
            ASTROPORT_LIQUIDITY_HELPER_WASM_FILE.to_string(),
        )),
        TestRunner::MultiTest(_) => {
            ContractType::MultiTestContract(Box::new(ContractWrapper::new(
                astroport_liquidity_helper::contract::execute,
                astroport_liquidity_helper::contract::instantiate,
                astroport_liquidity_helper::contract::query,
            )))
        }
        _ => panic!("Unsupported runner"),
    };
    let astroport_liquidity_helper_code_id = app
        .store_code(astroport_liquidity_helper_wasm_byte_code, admin)
        .unwrap();

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
    let owned_runner = get_test_runner();
    let runner = owned_runner.as_ref();
    let astroport_contracts =
        get_local_contracts(&runner, &Some("tests/astroport-artifacts"), false, &None);
    let admin = runner
        .init_account(&[
            coin(1_000_000_000_000_000_000_000_000_000_000_000u128, "uluna"),
            coin(1_000_000_000_000_000_000_000_000_000_000_000u128, "uosmo"),
        ])
        .unwrap();
    let astroport_contracts = &setup_astroport(&runner, astroport_contracts, &admin);
    let wasm = Wasm::new(&runner);

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
        &admin,
        None,
    );

    // Increase allowance of astro token for Pair contract
    let increase_allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: uluna_astro_pair_addr.clone(),
        amount: Uint128::from(3_000_000_000_000u128),
        expires: None,
    };
    let _res = wasm
        .execute(&astro_token, &increase_allowance_msg, &[], &admin)
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
        &admin,
    ).unwrap();

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
        calc_xyk_balancing_swap(assets, reserves,
total_fee_rate, None).unwrap();

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

const TOLERANCE: &str = "0.0005";

// Test 1: 1:1 ratio, double amount of asset 2
#[test_matrix(
    [[Uint128::from(1_000_000u128), Uint128::from(2_000_000u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string()), PairType::Custom("astroport-pair-xyk-sale-tax".to_string())],
    true
)]
// Test 2: 1:5 ratio, double amount of asset 2
#[test_matrix(
    [[Uint128::from(1_000_000u128), Uint128::from(2_000_000u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(5_000_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string()), PairType::Custom("astroport-pair-xyk-sale-tax".to_string())],
    true
)]
// Test 3: 1:2.9 pool ratio, 1:1 ratio of assets, but a lot of assets compared to pool (high
// slippage)
#[test_matrix(
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(2_900_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string()), PairType::Custom("astroport-pair-xyk-sale-tax".to_string())],
    true
)]
// Test 4: 1:2 pool ratio, 0:1 ratio of assets
#[test_matrix(
    [[Uint128::from(0u128), Uint128::from(1_000_000_000_000u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(2_000_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string()), PairType::Custom("astroport-pair-xyk-sale-tax".to_string())],
    true
)]
// Test 5: 1:1 pool ratio, 1:1 ratio of assets
#[test_matrix(
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string()), PairType::Custom("astroport-pair-xyk-sale-tax".to_string())],
    true
)]
// Test 6: 1:1 pool ratio, 1:0 ratio of assets
#[test_matrix(
    [[Uint128::from(1_000_000_000u128), Uint128::from(0u128)]],
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [PairType::Xyk {},PairType::Stable {}, PairType::Custom("concentrated".to_string()), PairType::Custom("astroport-pair-xyk-sale-tax".to_string())],
    true
)]
// Test 7: Xyk amount of asset less than one microunit of other asset
#[test_matrix(
    [[Uint128::from(0u128), Uint128::from(3564u128)]],
    [[Uint128::from(3450765745u128), Uint128::from(12282531965699u128)]],
    [PairType::Xyk {}, PairType::Custom("astroport-pair-xyk-sale-tax".to_string())],
    false
)]
// Test 7: Amount of asset would be less than one microunit of other asset if it were xyk
#[test_matrix(
    [[Uint128::from(0u128), Uint128::from(3564u128)]],
    [[Uint128::from(3450765745u128), Uint128::from(12282531965699u128)]],
    [PairType::Stable {  }, PairType::Custom("concentrated".to_string())],
    true
)]
// Test 8: Xyk 0:0 pool ratio, should fail with correct error
#[test_matrix(
    [[Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)]],
    [[Uint128::from(0u128), Uint128::from(0u128)]],
    [PairType::Xyk {}, PairType::Custom("astroport-pair-xyk-sale-tax".to_string())],
    true
    => panics "No liquidity in pool";
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
    let owned_runner = get_test_runner();
    let runner = owned_runner.as_ref();
    let astroport_contracts =
        get_local_contracts(&runner, &Some("tests/astroport-artifacts"), false, &None);
    let admin = runner
        .init_account(&[
            coin(1_000_000_000_000_000_000_000_000_000_000_000u128, "uluna"),
            coin(1_000_000_000_000_000_000_000_000_000_000_000u128, "uosmo"),
        ])
        .unwrap();
    let tax_recipient = runner.init_account(&[]).unwrap();
    let astroport_contracts = &setup_astroport(&runner, astroport_contracts, &admin);

    let wasm = Wasm::new(&runner);
    let liquidity_helper =
        setup_astroport_liquidity_provider_tests(&runner, astroport_contracts, &admin);
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
            to_json_binary(&StablePoolParams {
                amp: 10u64,
                owner: None,
            })
            .unwrap(),
        ),
        PairType::Custom(t) => match t.as_str() {
            "concentrated" => Some(to_json_binary(&common_pcl_params()).unwrap()),
            "astroport-pair-xyk-sale-tax" => Some(
                to_json_binary(&SaleTaxInitParams {
                    tax_config_admin: admin.address(),
                    track_asset_balances: false,
                    tax_configs: vec![(
                        "uluna",
                        TaxConfig {
                            tax_rate: Decimal::percent(3),
                            tax_recipient: tax_recipient.address(),
                        },
                    )]
                    .into(),
                })
                .unwrap(),
            ),
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
        &admin,
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
        liquidity_manager: Addr::unchecked(astroport_contracts.liquidity_manager.address.clone()),
    };

    // Increase allowance of astro token for Pair contract
    let increase_allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: uluna_astro_pair_addr.clone(),
        amount: reserves[1],
        expires: None,
    };
    let _res = wasm
        .execute(&astro_token, &increase_allowance_msg, &[], &admin)
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
        &admin,
    ).unwrap();

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
        .balancing_provide_liquidity(
            assets,
            Uint128::zero(),
            to_json_binary(&pool).unwrap(),
            None,
        )
        .unwrap();
    let _res = runner
        .execute_cosmos_msgs::<MsgExecuteContractResponse>(&msgs, &admin)
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
        let uluna_tax_amount = query_token_balance(&runner, &tax_recipient.address(), "uluna");
        let astro_tax_amount = query_cw20_balance(&runner, tax_recipient.address(), &astro_token);
        // Astroport liquidity manager rounds down the amount of tokens sent to the pool
        // by one unit.
        assert_approx_eq!(
            pool_liquidity[0].amount,
            reserves[0] + asset_amounts[0] - uluna_tax_amount,
            TOLERANCE
        );
        assert_approx_eq!(
            pool_liquidity[1].amount,
            reserves[1] + asset_amounts[1] - astro_tax_amount,
            TOLERANCE
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

pub fn f64_to_dec<T>(val: f64) -> T
where
    T: FromStr,
    T::Err: std::error::Error,
{
    T::from_str(&val.to_string()).unwrap()
}

pub fn common_pcl_params() -> ConcentratedPoolParams {
    ConcentratedPoolParams {
        amp: f64_to_dec(40f64),
        gamma: f64_to_dec(0.000145),
        mid_fee: f64_to_dec(0.0026),
        out_fee: f64_to_dec(0.0045),
        fee_gamma: f64_to_dec(0.00023),
        repeg_profit_threshold: f64_to_dec(0.000002),
        min_price_scale_delta: f64_to_dec(0.000146),
        price_scale: Decimal::one(),
        ma_half_time: 600,
        track_asset_balances: None,
    }
}
