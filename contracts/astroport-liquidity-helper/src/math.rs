//! Module containing implementations of calculations needed for swapping

use cosmwasm_std::{Decimal, Decimal256, StdError, StdResult, Uint128, Uint256};
use cw_asset::Asset;
use cw_bigint::BigInt;

/// Returns square root of a BigInt
fn bigint_sqrt(input: BigInt) -> StdResult<BigInt> {
    if input < 0.into() {
        return Err(StdError::generic_err(
            "Cannot calculate square root of negative number",
        ));
    }

    let mut x = input.clone();
    let mut y = (&x + 1u128) / 2u128;
    while y < x {
        x = y;
        y = (&x + &input / &x) / 2u128;
    }
    Ok(x)
}

/// Calculate how much will be returned from a swap in a constant product pool
fn constant_product_formula(
    offer_reserve: Uint128,
    ask_reserve: Uint128,
    offer_amount: Uint128,
    fee: Decimal,
) -> StdResult<Uint128> {
    let cp = offer_reserve.full_mul(ask_reserve);
    let return_amount: Uint256 = (Decimal256::from_ratio(ask_reserve, 1u8)
        - Decimal256::from_ratio(cp, offer_reserve + offer_amount))
        * Uint256::from(1u8);
    let commission_amount: Uint256 = return_amount * Decimal256::from(fee);
    let return_amount: Uint256 = return_amount - commission_amount;
    Ok(return_amount.try_into()?)
}

fn bigint_to_uint128(input: BigInt) -> StdResult<Uint128> {
    Ok(Uint128::from(TryInto::<u128>::try_into(input).map_err(
        |_| StdError::generic_err("Cannot convert BigInt to Uint128"),
    )?))
}

/// For a constant product pool, calculates how much of one asset we need to
/// swap to the other in order to have the same ratio of assets as the pool, so
/// that we can then provide liquidity and get the most amount of LP tokens.
///
/// Returns `(offer_asset, return_asset): (Asset,Asset)` containing the amount
/// and info of the asset we need to swap, and the asset that will be returned
/// from the swap
pub fn calc_xyk_balancing_swap(
    assets: [Asset; 2],
    reserve1: Uint128,
    reserve2: Uint128,
    fee: Decimal,
) -> StdResult<(Asset, Asset)> {
    // Instead of trying to implement our own big decimal, we just use BigInt
    // and multiply and divide with this number before and after doing
    // calculations.
    let precision: BigInt = BigInt::from(1_000_000_000u128);

    // Make sure there is liquidity in the pool
    if reserve1.is_zero() || reserve2.is_zero() {
        return Err(StdError::generic_err("No liquidity in pool"));
    }

    // Get ratio of reserves and provided assets
    let reserve_ratio = Decimal::from_ratio(reserve1, reserve2);
    let asset_ratio = Decimal::from_ratio(assets[0].amount, assets[1].amount);

    // Check which asset to swap
    let (offer_balance, ask_balance, offer_asset_info, ask_asset_info, offer_reserve, ask_reserve) =
        if asset_ratio.gt(&reserve_ratio) {
            (
                BigInt::from(assets[0].amount.u128()) * &precision,
                BigInt::from(assets[1].amount.u128()) * &precision,
                &assets[0].info,
                &assets[1].info,
                BigInt::from(reserve1.u128()) * &precision,
                BigInt::from(reserve2.u128()) * &precision,
            )
        } else {
            (
                BigInt::from(assets[1].amount.u128()) * &precision,
                BigInt::from(assets[0].amount.u128()) * &precision,
                &assets[1].info,
                &assets[0].info,
                BigInt::from(reserve2.u128()) * &precision,
                BigInt::from(reserve1.u128()) * &precision,
            )
        };

    let fee_int = (BigInt::from(fee.atomics().u128()) * &precision) / BigInt::from(10u128.pow(18));

    // Calculate amount to swap by solving quadratic equation
    let a = &ask_reserve + &ask_balance;
    let b = 2u128 * &offer_reserve * (&ask_reserve + &ask_balance)
        - ((&offer_reserve + &offer_balance) * &ask_reserve * &fee_int) / &precision;
    let c = &offer_reserve * (&offer_reserve * &ask_balance - &offer_balance * &ask_reserve);
    let discriminant = &b * &b - (4u128 * &a * &c);
    //  We know that for this equation, there is only one positive real solution
    let x = (bigint_sqrt(discriminant)? - b) / (2u128 * a);

    // Divide by precision to get final result and convert to Uint128
    let offer_amount = bigint_to_uint128(x / &precision)?;
    let offer_asset = Asset {
        amount: offer_amount,
        info: offer_asset_info.clone(),
    };

    // Calculate return amount from swap
    let return_amount = constant_product_formula(
        bigint_to_uint128(offer_reserve / &precision)?,
        bigint_to_uint128(ask_reserve / &precision)?,
        offer_amount,
        fee,
    )?;
    let return_asset = Asset {
        amount: return_amount,
        info: ask_asset_info.clone(),
    };

    Ok((offer_asset, return_asset))
}

#[cfg(test)]
mod test {
    use cosmwasm_std::{Decimal, Uint128};
    use cw_asset::{Asset, AssetInfo};
    use cw_bigint::BigInt;
    use test_case::test_case;

    use crate::math::{bigint_sqrt, calc_xyk_balancing_swap};

    /// Assert that two Decimals are almost the same (diff smaller than one permille)
    fn assert_decimal_almost_eq(a: Decimal, b: Decimal) {
        let diff = if a > b { a - b } else { b - a };
        if diff > Decimal::permille(1) {
            println!(
                "Failed assert decimal almost eq for a: {}, b: {}. diff: {}",
                a, b, diff
            );
            panic!();
        }
    }

    // Assert that the ratio of the users assets is the same as the pool after the swap
    fn assert_asset_ratios_same_after_swap(
        offer_reserve: Uint128,
        ask_reserve: Uint128,
        offer_balance: Uint128,
        ask_balance: Uint128,
        offer_amount: Uint128,
        return_amount: Uint128,
    ) {
        let asset_ratio_after_swap =
            Decimal::from_ratio(ask_balance + return_amount, offer_balance - offer_amount);
        let reserve_ratio_after_swap =
            Decimal::from_ratio(ask_reserve - return_amount, offer_reserve + offer_amount);
        println!(
            "asset_ratio_after_swap: {}, reserve_ratio_after_swap: {}",
            asset_ratio_after_swap, reserve_ratio_after_swap
        );
        assert_decimal_almost_eq(asset_ratio_after_swap, reserve_ratio_after_swap);
    }

    #[test_case(
        [Uint128::from(1_000_000u128), Uint128::from(2_000_000u128)],
        [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
        true,
        1;
        "Test 1: 1:1 ratio, double amount of asset 2"
    )]
    #[test_case(
        [Uint128::from(1_000_000u128), Uint128::from(2_000_000u128)],
        [Uint128::from(1_000_000_000_000u128), Uint128::from(5_000_000_000_000u128)],
        true,
        0;
        "Test 2: 1:5 ratio, double amount of asset 2"
    )]
    #[test_case(
        [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
        [Uint128::from(1_000_000_000_000u128), Uint128::from(3_000_000_000_000u128)],
        true,
        0;
        "Test 3: 1:3 pool ratio, 1:1 ratio of assets, but a lot of assets compared to pool (high slipage)"
    )]
    #[test_case(
        [Uint128::from(0u128), Uint128::from(1_000_000_000_000u128)],
        [Uint128::from(1_000_000_000_000u128), Uint128::from(2_000_000_000_000u128)],
        true,
        1;
        "Test 4: 1:2 pool ratio, 0:1 ratio of assets"
    )]
    #[test_case(
        [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
        [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
        false,
        1;
        "Test 5: 1:1 pool ratio, 1:1 ratio of assets"
    )]
    #[test_case(
        [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
        [Uint128::from(1_000_000_000_000u128), Uint128::from(0u128)],
        false,
        0
        => panics "No liquidity in pool";
        "Test 6: 1:0 pool ratio, should fail with correct error"
    )]
    #[test_case(
        [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
        [Uint128::from(0u128), Uint128::from(1_000_000_000_000u128)],
        false,
        0
        => panics "No liquidity in pool";
        "Test 7: 0:1 pool ratio, should fail with correct error"
    )]
    #[test_case(
        [Uint128::from(1_000_000_000_000u128), Uint128::from(0u128)],
        [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
        true,
        0;
        "Test 8: 1:1 pool ratio, 1:0 ratio of assets"
    )]
    fn test_calc_xyk_balancing_swap(
        assets: [Uint128; 2],
        reserves: [Uint128; 2],
        should_swap: bool,
        offer_asset_idx: usize,
    ) {
        let assets = [
            Asset {
                amount: assets[0],
                info: AssetInfo::native("uatom".to_string()),
            },
            Asset {
                amount: assets[1],
                info: AssetInfo::native("uosmo".to_string()),
            },
        ];
        let reserve1 = reserves[0];
        let reserve2 = reserves[1];
        let offer_asset = assets[offer_asset_idx].clone();
        let ask_asset = assets[1 - offer_asset_idx].clone();
        let offer_reserve = reserves[offer_asset_idx];
        let ask_reserve = reserves[1 - offer_asset_idx];

        // Same fee for all test cases
        let fee = Decimal::permille(3);

        println!("Assets: {:?}", assets);
        println!("Reserves: {}, {}", reserve1, reserve2);

        // Calculate swap
        let (swap_asset, return_asset) =
            calc_xyk_balancing_swap(assets.clone(), reserve1, reserve2, fee).unwrap();

        // If ratios are already almost the same, no swap should happen
        if !should_swap {
            assert_eq!(swap_asset.amount, Uint128::zero());
        }

        // Assert that the correct asset is being offered
        assert_eq!(swap_asset.info, offer_asset.info);

        // Assert that the asset ratio and the pool ratio are the same after the swap
        assert_asset_ratios_same_after_swap(
            offer_reserve,
            ask_reserve,
            offer_asset.amount,
            ask_asset.amount,
            swap_asset.amount,
            return_asset.amount,
        );
        println!("------------------------------------");
    }

    #[test]
    fn test_bigint_sqrt() {
        // Test the sqrt algorithm
        let test_cases = vec![
            (0, 0),
            (1, 1),
            (2, 1),
            (3, 1),
            (4, 2),
            (28, 5),
            (29, 5),
            (34, 5),
            (36, 6),
            (37, 6),
            (57, 7),
            (58, 7),
            (66, 8),
            (67, 8),
            (69, 8),
            (982734928374982u128, 31348603),
            (u128::MAX, 18446744073709551615u128),
        ];
        for (input, expected) in test_cases {
            let input = BigInt::from(input);
            let expected = BigInt::from(expected);
            let result = bigint_sqrt(input).unwrap();
            assert_eq!(result, expected);
        }

        // Some larger than u128::MAX test cases
        let test_cases = vec![
            (
                BigInt::from(u128::MAX) * 2,
                BigInt::from(26087635650665564424u128),
            ),
            (
                BigInt::from(u128::MAX) * 4,
                BigInt::from(36893488147419103231u128),
            ),
            (
                BigInt::from(u128::MAX) * 100,
                BigInt::from(184467440737095516159u128),
            ),
            (
                BigInt::from(u128::MAX) * 1000,
                BigInt::from(583337266871351588485u128),
            ),
        ];
        for (input, expected) in test_cases {
            let result = bigint_sqrt(input).unwrap();
            assert_eq!(result, expected);
        }
    }
}
