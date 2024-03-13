//! Module containing implementations of calculations needed for swapping

use apollo_cw_asset::Asset;
use astroport_v3::pair_xyk_sale_tax::TaxConfigsChecked;
use cosmwasm_std::{Decimal, Decimal256, Int256, StdError, StdResult, Uint128, Uint256};

fn mul(a: Int256, b: Decimal256) -> Int256 {
    match a.cmp(&Int256::zero()) {
        std::cmp::Ordering::Equal => Int256::zero(),
        std::cmp::Ordering::Greater => {
            let a = Uint256::try_from(a).unwrap();
            let b = a * b;

            Int256::try_from(b).unwrap()
        }
        std::cmp::Ordering::Less => {
            let a = Uint256::try_from(-a).unwrap();
            let b = a * b;

            -Int256::try_from(b).unwrap()
        }
    }
}

/// Returns square root of a BigInt
fn int256_sqrt(input: Int256) -> StdResult<Int256> {
    if input < 0.into() {
        return Err(StdError::generic_err(
            "Cannot calculate square root of negative number",
        ));
    }

    let mut x = input.clone();
    let mut y = (x + Int256::one()) / Int256::from(2u128);
    while y < x {
        x = y;
        y = (&x + &input / &x) / Int256::from(2u128);
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

/// For a constant product pool, calculates how much of one asset we need to
/// swap to the other in order to have the same ratio of assets as the pool, so
/// that we can then provide liquidity and get the most amount of LP tokens.
///
/// Returns `(offer_asset, return_asset): (Asset,Asset)` containing the amount
/// and info of the asset we need to swap, and the asset that will be returned
/// from the swap
pub fn calc_xyk_balancing_swap(
    assets: [Asset; 2],
    reserves: [Uint128; 2],
    fee: Decimal,
    tax_configs: Option<TaxConfigsChecked>,
) -> StdResult<(Asset, Asset)> {
    // Make sure there is liquidity in the pool
    if reserves[0].is_zero() || reserves[1].is_zero() {
        return Err(StdError::generic_err("No liquidity in pool"));
    }

    // Get ratio of reserves and provided assets
    let reserve_ratio = Decimal::from_ratio(reserves[0], reserves[1]);
    let asset_ratio = if assets[1].amount.is_zero() {
        Decimal::MAX
    } else {
        Decimal::from_ratio(assets[0].amount, assets[1].amount)
    };

    // Check which asset to swap
    let (offer_idx, ask_idx) = if asset_ratio.gt(&reserve_ratio) {
        (0, 1)
    } else {
        (1, 0)
    };
    let offer_reserve = Int256::from(reserves[offer_idx].u128());
    let ask_reserve = Int256::from(reserves[ask_idx].u128());
    let offer_balance = Int256::from(assets[offer_idx].amount.u128());
    let ask_balance = Int256::from(assets[ask_idx].amount.u128());

    let fee_rate = Decimal256::from(fee);

    // Unwrap tax
    let offer_asset_info = &assets[offer_idx].info;
    let tax_rate = tax_configs
        .map(|tax_configs| {
            tax_configs
                .get(&offer_asset_info.to_string())
                .map(|tax_config| tax_config.tax_rate)
                .unwrap_or(Decimal::zero())
        })
        .unwrap_or(Decimal::zero());
    let tax_rate = Decimal256::from(tax_rate);

    // Solve equation to find amount to swap
    let two = Int256::from(2u128);
    let four = two * two;
    let numerator = mul(offer_reserve * ask_reserve, fee_rate - fee_rate * tax_rate)
        + mul((offer_balance + offer_reserve) * ask_reserve, fee_rate)
        - two * offer_reserve * (ask_balance + ask_reserve);
    println!("numerator: {numerator}");
    let discriminant = (two * &offer_reserve * &ask_balance
        - mul(offer_balance * ask_reserve, fee_rate)
        + two * mul(offer_reserve * ask_reserve, Decimal256::one() - fee_rate)
        + mul(offer_reserve * ask_reserve, fee_rate * tax_rate))
    .pow(2)
        - four
            * (ask_balance + ask_reserve + mul(ask_reserve, fee_rate * tax_rate - tax_rate))
            * (offer_reserve.pow(2) * ask_balance - offer_balance * offer_reserve * ask_reserve);
    println!("discriminant: {discriminant}");
    let denominator = two
        * (ask_balance + ask_reserve - mul(ask_reserve, tax_rate)
            + mul(ask_reserve, fee_rate * tax_rate));

    println!("denominator: {denominator}");

    let x = (numerator + int256_sqrt(discriminant)?) / denominator;

    println!("x: {x}");

    // Divide by precision to get final result and convert to Uint128
    let offer_amount: Uint128 = x.try_into()?;
    let offer_asset = Asset {
        amount: offer_amount,
        info: assets[offer_idx].info.clone(),
    };

    // Calculate return amount from swap
    let return_amount = constant_product_formula(
        offer_reserve.try_into()?,
        ask_reserve.try_into()?,
        offer_amount,
        fee,
    )?;
    let return_asset = Asset {
        amount: return_amount,
        info: assets[ask_idx].info.clone(),
    };

    Ok((offer_asset, return_asset))
}

#[cfg(test)]
mod test {
    use apollo_cw_asset::{Asset, AssetInfo};
    use cosmwasm_std::{Decimal, Uint128};
    use cw_bigint::BigInt;
    use test_case::test_case;

    use crate::math::calc_xyk_balancing_swap;

    /// Assert that two Decimals are almost the same (diff smaller than one
    /// permille)
    fn assert_decimal_almost_eq(a: Decimal, b: Decimal) {
        let diff = if a > b { (a - b) / a } else { (b - a) / b };
        if diff > Decimal::permille(1) {
            println!("Failed assert decimal almost eq for a: {a}, b: {b}. diff: {diff}");
            panic!();
        }
    }

    // Assert that the ratio of the users assets is the same as the pool after the
    // swap
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
            "asset_ratio_after_swap: {asset_ratio_after_swap}, reserve_ratio_after_swap: {reserve_ratio_after_swap}"
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
    #[test_case(
        [Uint128::from(0u128), Uint128::from(3564u128)],
        [Uint128::from(3450765745u128), Uint128::from(12282531965699u128)],
        true,
        1;
        "Test 9: Amount of asset less than one microunit of other asset"
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
        let offer_asset = assets[offer_asset_idx].clone();
        let ask_asset = assets[1 - offer_asset_idx].clone();
        let offer_reserve = reserves[offer_asset_idx];
        let ask_reserve = reserves[1 - offer_asset_idx];

        // Same fee for all test cases
        let fee = Decimal::permille(3);

        println!("Assets: {assets:?}");
        println!("Reserves: {reserves:?}");

        // Calculate swap
        let (swap_asset, return_asset) =
            calc_xyk_balancing_swap(assets, reserves, fee, None).unwrap();

        println!("Swap: {swap_asset:?}, Return: {return_asset:?}");

        // If ratios are already almost the same, no swap should happen
        if !should_swap {
            assert_eq!(swap_asset.amount, Uint128::zero());
        }

        // Assert that the correct asset is being offered
        assert_eq!(swap_asset.info, offer_asset.info);

        // If the amount returned is zero because the swapped amount is too small
        // then the following assert will fail, so we just return here
        if return_asset.amount == Uint128::zero() {
            return;
        }

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
}
