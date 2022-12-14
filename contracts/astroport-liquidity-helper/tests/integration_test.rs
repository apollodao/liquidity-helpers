use astroport::asset::{Asset as AstroAsset, AssetInfo as AstroAssetInfo};
use astroport::factory::{FeeInfoResponse, PairType, QueryMsg as FactoryQueryMsg};

use astroport::pair::ExecuteMsg as PairExecuteMsg;
use astroport::pair::QueryMsg as PairQueryMsg;
use astroport_liquidity_helper::math::calc_xyk_balancing_swap;
use astroport_liquidity_helper::msg::InstantiateMsg;
use cosmwasm_std::{to_binary, Addr, Coin, Decimal, Uint128};
use cw20::{AllowanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
use cw_asset::{Asset, AssetInfo, AssetList};
use cw_dex::astroport::msg::{PoolResponse, SimulationResponse};
use cw_dex::astroport::AstroportPool;
use cw_it::astroport::AstroportContracts;
use cw_it::astroport::{create_astroport_pair, instantiate_astroport, upload_astroport_contracts};
use cw_it::config::TestConfig;
// use cw_it::{app::App as RpcRunner, Cli};
use liquidity_helper::LiquidityHelper;

use osmosis_testing::OsmosisTestApp;
use osmosis_testing::{
    cosmrs::proto::cosmwasm::wasm::v1::MsgExecuteContractResponse, Account, Module, Runner,
    SigningAccount, Wasm,
};
use std::collections::HashMap;
use std::str::FromStr;

const TEST_CONFIG_PATH: &str = "tests/configs/terra.yaml";
pub const ASTROPORT_LIQUIDITY_HELPER_WASM_FILE: &str =
    "../../artifacts/astroport_liquidity_helper.wasm";

#[test]
/// Runs all tests against the Osmosis bindings.
/// This works since there are no big differences between the chains.
pub fn test_with_osmosis_bindings() {
    let app = OsmosisTestApp::default();

    let accs = app
        .init_accounts(&[Coin::new(1_000_000_000_000_000, "uluna")], 2)
        .unwrap();

    let test_config = TestConfig::from_yaml(TEST_CONFIG_PATH);

    // Upload astroport contracts
    let astroport_code_ids = upload_astroport_contracts(&app, &test_config, &accs[0]);

    test_balancing_provide_liquidity(&app, &accs, &astroport_code_ids);
    test_calc_xyk_balancing_swap(&app, &accs, &astroport_code_ids);
}

// Commented out for now since LocalTerra does not support cosmwasm-std feature
// "cosmwasm_1_1", which is enabled by dev-dependency osmosis-testing.
// #[test]
// /// Runs all tests against LocalTerra
// pub fn test_with_localterra() {
//     // let _ = env_logger::builder().is_test(true).try_init();
//     let docker: Cli = Cli::default();
//     let test_config = TestConfig::from_yaml(TEST_CONFIG_PATH);
//     let app = RpcRunner::new(test_config.clone(), &docker);

//     let accs = app
//         .test_config
//         .import_all_accounts()
//         .into_values()
//         .collect::<Vec<_>>();

//     // Upload astroport contracts
//     let astroport_code_ids = upload_astroport_contracts(&app, &test_config, &accs[0]);

//     // Test basic liquidity helper functionality
//     test_balancing_provide_liquidity(&app, &accs, &astroport_code_ids);
//     test_calc_xyk_balancing_swap(&app, &accs, &astroport_code_ids);
// }

/// Instantiates the liquidity helper contract
pub fn setup_astroport_liquidity_provider_tests<R>(
    app: &R,
    accs: &[SigningAccount],
    astroport_contracts: &AstroportContracts,
) -> LiquidityHelper
where
    R: for<'a> Runner<'a>,
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

pub fn test_calc_xyk_balancing_swap<'a, R>(
    app: &'a R,
    accs: &[SigningAccount],
    astroport_code_ids: &HashMap<String, u64>,
) where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);
    let admin = &accs[0];

    // Instantiate Astroport contracts
    let astroport_contracts = instantiate_astroport(app, admin, astroport_code_ids);

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
        app,
        &astroport_contracts.factory.address,
        PairType::Xyk {},
        asset_infos.clone(),
        None,
        admin,
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
        ],
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
    let reserve1 = Uint128::from(1_000_000_000_000u128);
    let reserve2 = Uint128::from(3_000_000_000_000u128);

    let (offer_asset, return_asset) =
        calc_xyk_balancing_swap(assets, reserve1, reserve2, total_fee_rate).unwrap();

    // Simulate swap
    let simulation_result: SimulationResponse = wasm
        .query(
            &uluna_astro_pair_addr,
            &PairQueryMsg::Simulation {
                offer_asset: AstroAsset {
                    amount: offer_asset.amount,
                    info: asset_infos[0].clone(),
                },
            },
        )
        .unwrap();

    // Check if the simulation result is correct
    assert_eq!(simulation_result.return_amount, return_asset.amount);
}

/// Tests the BalancingProvideLiquidity message
pub fn test_balancing_provide_liquidity<R>(
    app: &R,
    accs: &[SigningAccount],
    astroport_code_ids: &HashMap<String, u64>,
) where
    R: for<'a> Runner<'a>,
{
    let admin = &accs[0];
    let wasm = Wasm::new(app);

    // Instantiate Astroport contracts
    let astroport_contracts = instantiate_astroport(app, admin, astroport_code_ids);

    let liquidity_helper =
        setup_astroport_liquidity_provider_tests(app, accs, &astroport_contracts);
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
    let (uluna_astro_pair_addr, uluna_astro_lp_token) = create_astroport_pair(
        app,
        &astroport_contracts.factory.address,
        PairType::Xyk {},
        asset_infos,
        None,
        admin,
    );
    let pool = AstroportPool {
        lp_token_addr: Addr::unchecked(uluna_astro_lp_token),
        pair_addr: Addr::unchecked(uluna_astro_pair_addr.clone()),
        pair_type: cw_dex::astroport::msg::PairType::Xyk {},
        pool_assets: vec![
            AssetInfo::native("uluna".to_string()),
            AssetInfo::cw20(Addr::unchecked(&astro_token)),
        ],
    };

    // Increase allowance of astro token for Pair contract
    let increase_allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: uluna_astro_pair_addr.clone(),
        amount: Uint128::from(1000000000u128),
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
    assert_eq!(allowance_res.allowance, Uint128::from(1000000000u128));

    // Provide liquidity normal to have some liquidity in pool
    let provide_liq_msg = PairExecuteMsg::ProvideLiquidity {
        assets: [
            AstroAsset {
                amount: Uint128::from(1000000000u128),
                info: AstroAssetInfo::NativeToken {
                    denom: "uluna".into(),
                },
            },
            AstroAsset {
                amount: Uint128::from(1000000000u128),
                info: AstroAssetInfo::Token {
                    contract_addr: Addr::unchecked(&astro_token),
                },
            },
        ],
        slippage_tolerance: Some(Decimal::from_str("0.02").unwrap()),
        auto_stake: Some(false),
        receiver: None,
    };
    let _res = wasm.execute(
        &uluna_astro_pair_addr,
        &provide_liq_msg,
        &[Coin {
            amount: Uint128::from(1000000000u128),
            denom: "uluna".into(),
        }],
        admin,
    );

    // Balancing Provide liquidity
    println!("Balancing provide liquidity");
    let mut assets: AssetList = vec![Coin::new(100_000, "uluna")].into();
    assets
        .add(&Asset::new(
            AssetInfo::Cw20(Addr::unchecked(&astro_token)),
            Uint128::from(100_000u128),
        ))
        .unwrap();
    let msgs = liquidity_helper
        .balancing_provide_liquidity(assets, Uint128::one(), to_binary(&pool).unwrap(), None)
        .unwrap();

    let _res = app
        .execute_cosmos_msgs::<MsgExecuteContractResponse>(&msgs, admin)
        .unwrap();

    // Check pool liquidity after adding
    let mut initial_pool_liquidity = AssetList::new();
    initial_pool_liquidity
        .add(&Asset::native("uluna", Uint128::from(1000000000u128)))
        .unwrap()
        .add(&Asset::new(
            AssetInfo::Cw20(Addr::unchecked(&astro_token)),
            Uint128::from(1000000000u128),
        ))
        .unwrap();
    let expected_liquidity_after_add = initial_pool_liquidity
        .add(&Asset::native("uluna", Uint128::from(100_000u128)))
        .unwrap()
        .add(&Asset::new(
            AssetInfo::Cw20(Addr::unchecked(&astro_token)),
            Uint128::from(100_000u128),
        ))
        .unwrap();
    let pool_liquidity: PoolResponse = wasm
        .query(&uluna_astro_pair_addr, &PairQueryMsg::Pool {})
        .unwrap();
    let pool_liquidity: AssetList = pool_liquidity.assets.to_vec().into();
    assert_eq!(&pool_liquidity, expected_liquidity_after_add);
}
