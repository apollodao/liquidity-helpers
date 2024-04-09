
pub mod astroport_integration_tests;

use cosmwasm_std::{Uint128,Decimal};
use cw_dex_astroport::astroport::factory::PairType;
use proptest::prelude::*;
use proptest::proptest;
use astroport_integration_tests::test_balancing_provide_liquidity;
use astroport_liquidity_helper::math::constant_product_formula;

fn astroport_pair_type() -> impl Strategy<Value = PairType> {
    prop_oneof![
        Just(PairType::Xyk {}),
        Just(PairType::Stable {}),
        // Just(PairType::Custom("concentrated".to_string())), // Errors with `newton_d is not converging`
        Just(PairType::Custom("astroport-pair-xyk-sale-tax".to_string())),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_local_rejects: 100000,
        max_global_rejects: 100000,
        max_shrink_iters: 5,
        ..ProptestConfig::default()
    })]

    #[test]
    fn balancing_provide_liquidity(
        asset_amount_1 in 1..Decimal::MAX.to_uint_ceil().u128(), // cw_dex_astroport::pool::swap overflows on line 235 with larger values...
        asset_amount_2 in 1..100_000_000_000_000u128,
        reserve_1 in 1..1_000_000_000_000_000_000_000u128,
        reserve_2 in 1..100_000_000_000_000u128,
        pair_type in astroport_pair_type())
    {
        let reserves = [Uint128::new(reserve_1), Uint128::new(reserve_2)];
        let assets = [Uint128::new(asset_amount_1), Uint128::new(asset_amount_2)];

        let is_xyk = match &pair_type {
            PairType::Xyk {} => true,
            PairType::Custom(t) if t == "astroport-pair-xyk-sale-tax" => true,
            _ => false,
        };

        let should_swap = if is_xyk {
            // Get ratio of reserves and provided assets
            let reserve_ratio = Decimal::from_ratio(reserves[0], reserves[1]);
            let asset_ratio = if assets[1].is_zero() {
                Decimal::MAX
            } else {
                Decimal::from_ratio(assets[0], assets[1])
            };

            // Check which asset to swap
            let (offer_idx, ask_idx) = if asset_ratio.gt(&reserve_ratio) {
                (0, 1)
            } else {
                (1, 0)
            };
            let offer_reserve = reserves[offer_idx];
            let ask_reserve = reserves[ask_idx];
            let offer_amount = assets[offer_idx];
            let fee = Decimal::permille(30);
            let tax_rate = if matches!(pair_type, PairType::Xyk{}) {
                Decimal::percent(3)
            } else {
                Decimal::percent(0)
            };

            let return_amount = constant_product_formula(offer_reserve, ask_reserve, offer_amount, fee, tax_rate).unwrap();
            return_amount > Uint128::zero()
        } else {
            true
        };

        test_balancing_provide_liquidity(
            assets,
            reserves,
            pair_type,
            should_swap
        );
    }

}
