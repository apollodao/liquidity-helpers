//! Module containing implementations of calculations needed for swapping

use apollo_cw_asset::Asset;
use astroport_v3::pair_xyk_sale_tax::TaxConfigsChecked;
use cosmwasm_std::{Decimal, Deps, Int256, StdError, StdResult, Uint128, Uint256};
use cw_bigint::BigInt;

use crate::math::big_decimal::{bigint_to_u128, BigDecimal};

pub mod big_decimal {
    use std::{
        fmt::Display,
        ops::{Add, Deref, Div, Mul, Sub},
    };

    use cosmwasm_std::{Decimal, Fraction, StdError, StdResult, Uint128};
    use cw_bigint::BigInt;

    pub const BIG_DECIMAL_FRACTIONAL: u128 = 1_000_000_000_000_000_000;

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct BigDecimal(BigInt);

    impl BigDecimal {
        pub fn new(value: BigInt) -> Self {
            Self(value)
        }

        pub fn zero() -> Self {
            Self(BigInt::from(0u128))
        }

        pub fn one() -> Self {
            Self(BigInt::from(BIG_DECIMAL_FRACTIONAL))
        }

        pub fn atomics(&self) -> BigInt {
            self.0.clone()
        }

        /// Returns the square root of the BigDecimal.
        ///
        /// Uses the Newton-Raphson method to approximate the square root.
        ///
        /// # Panics
        /// If the BigDecimal is negative, this function will panic.
        pub fn sqrt(&self) -> Self {
            if self < &Self::zero() {
                panic!("Cannot compute the square root of a negative number.");
            }
            if self == &Self::zero() {
                return Self::zero();
            }

            let mut x = self.clone();
            let mut y = (x.clone() + Self::one()) / BigDecimal::from(2u128);
            println!(
                "x: {}, y: {}",
                u128::try_from(x.0.clone()).unwrap(),
                u128::try_from(y.0.clone()).unwrap()
            );
            while y < x {
                x = y.clone();
                y = (x.clone() + self.clone() / x.clone()) / BigDecimal::from(2u128);
                println!(
                    "x: {}, y: {}",
                    u128::try_from(x.0.clone()).unwrap(),
                    u128::try_from(y.0.clone()).unwrap()
                );
            }
            y
        }

        pub fn pow(&self, exp: u32) -> Self {
            if exp == 0 {
                return Self::one();
            }
            if exp == 1 {
                return self.clone();
            }

            // BigDecimal is a fixed-point number with BIG_DECIMAL_FRACTIONAL decimal places.
            // x^y = (numerator / denominator)^y = numerator^y / denominator^y
            //     = (numerator^y / denominator^(y-1)) / denominator
            // which means we represent the new number as new_numerator = numerator^y / denominator^(y-1),
            // and new_denominator = denominator.
            let value: BigInt = self.0.pow(exp) / BIG_DECIMAL_FRACTIONAL.pow(exp - 1);

            Self(value)
        }

        pub fn floor(&self) -> BigInt {
            self.0.clone() / BIG_DECIMAL_FRACTIONAL
        }
    }

    // impl Display for BigDecimal {
    //     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    //         let bigint_decimal_str = self.0.to_str_radix(10);
    //         let str_len = bigint_decimal_str.len();
    //         if str_len <= 18 {
    //             // Pad with zeros if length is less than 18
    //             let num_of_zeroes = 18 - str_len;
    //             write!(f, "0.{}{}", "0".repeat(num_of_zeroes), bigint_decimal_str)
    //         } else {
    //             let (integer_part, fractional_part) = bigint_decimal_str.split_at(str_len - 18);
    //             write!(f, "{}.{}", integer_part, fractional_part)
    //         }
    //     }
    // }

    impl Mul for BigDecimal {
        type Output = Self;

        fn mul(self, rhs: Self) -> Self::Output {
            Self(self.0 * rhs.0 / BIG_DECIMAL_FRACTIONAL)
        }
    }

    impl<'a, 'b> Mul<&'b BigDecimal> for &'a BigDecimal {
        type Output = BigDecimal;

        fn mul(self, rhs: &'b BigDecimal) -> Self::Output {
            BigDecimal(self.0.clone() * rhs.0.clone() / BIG_DECIMAL_FRACTIONAL)
        }
    }

    impl<'a> Mul<BigDecimal> for &'a BigDecimal {
        type Output = BigDecimal;

        fn mul(self, rhs: BigDecimal) -> Self::Output {
            BigDecimal(self.0.clone() * rhs.0 / BIG_DECIMAL_FRACTIONAL)
        }
    }

    impl<'a> Mul<&'a BigDecimal> for BigDecimal {
        type Output = BigDecimal;

        fn mul(self, rhs: &'a BigDecimal) -> Self::Output {
            BigDecimal(self.0 * rhs.0.clone() / BIG_DECIMAL_FRACTIONAL)
        }
    }

    impl Mul<BigInt> for BigDecimal {
        type Output = Self;

        fn mul(self, rhs: BigInt) -> Self::Output {
            Self(self.0 * rhs)
        }
    }

    impl<'a> Mul<&'a BigInt> for BigDecimal {
        type Output = Self;

        fn mul(self, rhs: &'a BigInt) -> Self::Output {
            Self(self.0 * rhs.clone())
        }
    }

    impl<'a> Mul<BigInt> for &'a BigDecimal {
        type Output = BigDecimal;

        fn mul(self, rhs: BigInt) -> Self::Output {
            BigDecimal(self.0.clone() * rhs)
        }
    }

    impl<'a, 'b> Mul<&'b BigInt> for &'a BigDecimal {
        type Output = BigDecimal;

        fn mul(self, rhs: &'b BigInt) -> Self::Output {
            BigDecimal(self.0.clone() * rhs.clone())
        }
    }

    impl Mul<BigDecimal> for BigInt {
        type Output = BigDecimal;

        fn mul(self, rhs: BigDecimal) -> Self::Output {
            BigDecimal(self * rhs.0)
        }
    }

    impl<'a> Mul<&'a BigDecimal> for BigInt {
        type Output = BigDecimal;

        fn mul(self, rhs: &'a BigDecimal) -> Self::Output {
            BigDecimal(self * rhs.0.clone())
        }
    }

    impl<'a> Mul<BigDecimal> for &'a BigInt {
        type Output = BigDecimal;

        fn mul(self, rhs: BigDecimal) -> Self::Output {
            BigDecimal(self.clone() * rhs.0)
        }
    }

    impl<'a, 'b> Mul<&'a BigDecimal> for &'b BigInt {
        type Output = BigDecimal;

        fn mul(self, rhs: &'a BigDecimal) -> Self::Output {
            BigDecimal(self.clone() * rhs.0.clone())
        }
    }

    impl Add for BigDecimal {
        type Output = Self;

        fn add(self, rhs: Self) -> Self::Output {
            Self(self.0 + rhs.0)
        }
    }

    impl<'a, 'b> Add<&'b BigDecimal> for &'a BigDecimal {
        type Output = BigDecimal;

        fn add(self, rhs: &'b BigDecimal) -> Self::Output {
            BigDecimal(self.0.clone() + rhs.0.clone())
        }
    }

    impl<'a> Add<BigDecimal> for &'a BigDecimal {
        type Output = BigDecimal;

        fn add(self, rhs: BigDecimal) -> Self::Output {
            BigDecimal(self.0.clone() + rhs.0)
        }
    }

    impl<'a> Add<&'a BigDecimal> for BigDecimal {
        type Output = BigDecimal;

        fn add(self, rhs: &'a BigDecimal) -> Self::Output {
            BigDecimal(self.0 + rhs.0.clone())
        }
    }

    impl Add<BigInt> for BigDecimal {
        type Output = Self;

        fn add(self, rhs: BigInt) -> Self::Output {
            Self(self.0 + rhs * BIG_DECIMAL_FRACTIONAL)
        }
    }

    impl<'a, 'b> Add<&'b BigInt> for &'a BigDecimal {
        type Output = BigDecimal;

        fn add(self, rhs: &'b BigInt) -> Self::Output {
            BigDecimal(self.0.clone() + rhs.clone() * BIG_DECIMAL_FRACTIONAL)
        }
    }

    impl Add<BigDecimal> for BigInt {
        type Output = BigDecimal;

        fn add(self, rhs: BigDecimal) -> Self::Output {
            rhs + self
        }
    }

    impl<'a> Add<&'a BigDecimal> for BigInt {
        type Output = BigDecimal;

        fn add(self, rhs: &'a BigDecimal) -> Self::Output {
            rhs + self
        }
    }

    impl<'a> Add<BigInt> for &'a BigDecimal {
        type Output = BigDecimal;

        fn add(self, rhs: BigInt) -> Self::Output {
            self.clone() + rhs
        }
    }

    impl<'a, 'b> Add<&'a BigDecimal> for &'b BigInt {
        type Output = BigDecimal;

        fn add(self, rhs: &'a BigDecimal) -> Self::Output {
            rhs + self.clone()
        }
    }

    impl Sub for BigDecimal {
        type Output = Self;

        fn sub(self, rhs: Self) -> Self::Output {
            Self(self.0 - rhs.0)
        }
    }

    impl<'a, 'b> Sub<&'b BigDecimal> for &'a BigDecimal {
        type Output = BigDecimal;

        fn sub(self, rhs: &'b BigDecimal) -> Self::Output {
            BigDecimal(self.0.clone() - rhs.0.clone())
        }
    }

    impl<'a> Sub<BigDecimal> for &'a BigDecimal {
        type Output = BigDecimal;

        fn sub(self, rhs: BigDecimal) -> Self::Output {
            BigDecimal(self.0.clone() - rhs.0)
        }
    }

    impl<'a> Sub<&'a BigDecimal> for BigDecimal {
        type Output = BigDecimal;

        fn sub(self, rhs: &'a BigDecimal) -> Self::Output {
            BigDecimal(self.0 - rhs.0.clone())
        }
    }

    impl Sub<BigInt> for BigDecimal {
        type Output = Self;

        fn sub(self, rhs: BigInt) -> Self::Output {
            Self(self.0 - rhs * BIG_DECIMAL_FRACTIONAL)
        }
    }

    impl<'a, 'b> Sub<&'a BigInt> for &'b BigDecimal {
        type Output = BigDecimal;

        fn sub(self, rhs: &'a BigInt) -> Self::Output {
            BigDecimal(self.0.clone() - rhs.clone() * BIG_DECIMAL_FRACTIONAL)
        }
    }

    impl Sub<BigDecimal> for BigInt {
        type Output = BigDecimal;

        fn sub(self, rhs: BigDecimal) -> Self::Output {
            BigDecimal(self * BIG_DECIMAL_FRACTIONAL - rhs.0)
        }
    }

    impl<'a, 'b> Sub<&'a BigDecimal> for &'b BigInt {
        type Output = BigDecimal;

        fn sub(self, rhs: &'a BigDecimal) -> Self::Output {
            BigDecimal(self.clone() * BIG_DECIMAL_FRACTIONAL - rhs.0.clone())
        }
    }

    impl Div for BigDecimal {
        type Output = Self;

        fn div(self, rhs: Self) -> Self::Output {
            Self(self.0 * BIG_DECIMAL_FRACTIONAL / rhs.0)
        }
    }

    impl<'a, 'b> Div<&'b BigDecimal> for &'a BigDecimal {
        type Output = BigDecimal;

        fn div(self, rhs: &'b BigDecimal) -> Self::Output {
            BigDecimal(self.0.clone() * BIG_DECIMAL_FRACTIONAL / rhs.0.clone())
        }
    }

    impl<'a> Div<BigDecimal> for &'a BigDecimal {
        type Output = BigDecimal;

        fn div(self, rhs: BigDecimal) -> Self::Output {
            BigDecimal(self.0.clone() * BIG_DECIMAL_FRACTIONAL / rhs.0)
        }
    }

    impl<'a> Div<&'a BigDecimal> for BigDecimal {
        type Output = BigDecimal;

        fn div(self, rhs: &'a BigDecimal) -> Self::Output {
            BigDecimal(self.0 * BIG_DECIMAL_FRACTIONAL / rhs.0.clone())
        }
    }

    impl From<BigInt> for BigDecimal {
        fn from(value: BigInt) -> Self {
            Self(value * BIG_DECIMAL_FRACTIONAL)
        }
    }

    impl From<u128> for BigDecimal {
        fn from(value: u128) -> Self {
            Self(BigInt::from(value) * BIG_DECIMAL_FRACTIONAL)
        }
    }

    impl From<Decimal> for BigDecimal {
        fn from(value: Decimal) -> Self {
            let value =
                BigInt::from(value.numerator().u128()) * BigInt::from(value.denominator().u128());
            Self(value / BIG_DECIMAL_FRACTIONAL)
        }
    }

    impl TryFrom<BigDecimal> for Decimal {
        type Error = StdError;

        fn try_from(value: BigDecimal) -> Result<Self, Self::Error> {
            let numerator: Uint128 = bigint_to_u128(&value.0)?.into();
            Ok(Decimal::from_ratio(numerator, BIG_DECIMAL_FRACTIONAL))
        }
    }

    pub fn bigint_to_u128(big_int: &BigInt) -> StdResult<u128> {
        let (sign, bytes) = big_int.to_bytes_be();

        if sign == cw_bigint::Sign::Minus {
            return Err(StdError::generic_err(
                "Cannot convert a negative BigInt number to u128",
            ));
        }

        if bytes.len() > 16 {
            return Err(StdError::generic_err(
                "Attempting to convert BigInt to u128 with overflow",
            ));
        }

        // Pad with zeros if length is less than 16
        let num_of_zero_bytes = 16 - bytes.len();
        let mut padded_bytes = vec![0u8; num_of_zero_bytes];
        padded_bytes.extend_from_slice(&bytes);

        Ok(u128::from_be_bytes(
            padded_bytes
                .as_slice()
                .try_into()
                .map_err(|e| StdError::generic_err(format!("Failed to convert to slice: {e}")))?,
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::BIG_DECIMAL_FRACTIONAL;

        use super::{bigint_to_u128, BigDecimal};
        use cosmwasm_std::{StdError, StdResult};
        use cw_bigint::BigInt;
        use test_case::test_case;

        const BIG_DECIMAL_FRACTIONAL_I128: i128 = BIG_DECIMAL_FRACTIONAL as i128;

        // #[test_case(0u128 => "0.000000000000000000"; "zero")]
        // #[test_case(BIG_DECIMAL_FRACTIONAL => "1.000000000000000000"; "one")]
        // fn test_bigdecimal_to_string(val: u128) -> String {
        //     BigDecimal::new(val.into()).to_string()
        // }

        #[test_case(0u128, 0u128 => Ok(0u128); "zero")]
        #[test_case(1u128, 0u128 => Ok(1u128); "one")]
        #[test_case(u128::MAX, 0u128 => Ok(u128::MAX); "u128::MAX")]
        #[test_case(u128::MAX, 1u128 => Err(StdError::generic_err("Attempting to convert BigInt to u128 with overflow")); "u128::MAX + 1")]
        fn test_bigint_to_u128(value1: u128, value2: u128) -> StdResult<u128> {
            let big_int = BigInt::from(value1) + BigInt::from(value2);
            bigint_to_u128(&big_int)
        }

        #[test]
        fn test_bigint_to_u128_negative() {
            let big_int = BigInt::from(-1);
            let result = bigint_to_u128(&big_int);
            assert_eq!(
                result,
                Err(StdError::generic_err(
                    "Cannot convert a negative BigInt number to u128"
                ))
            );
        }

        #[test_case(0u128, 0u128, 0u128; "zero plus zero")]
        #[test_case(BIG_DECIMAL_FRACTIONAL, 0u128, BIG_DECIMAL_FRACTIONAL; "one plus zero")]
        #[test_case(0u128, BIG_DECIMAL_FRACTIONAL, BIG_DECIMAL_FRACTIONAL; "zero plus one")]
        #[test_case(BIG_DECIMAL_FRACTIONAL, BIG_DECIMAL_FRACTIONAL, 2 * BIG_DECIMAL_FRACTIONAL; "one plus one")]
        #[test_case(12346u128, 45678u128, 58024u128; "0.000000000000012346 plus 0.000000000000045678")]
        fn test_bigdecimal_add_bigdecimal(a: u128, b: u128, expected: u128) {
            let a = BigDecimal::new(a.into());
            let b = BigDecimal::new(b.into());
            let expected = BigDecimal::new(expected.into());
            assert_eq!(&a + &b, expected);
            assert_eq!(&a + b.clone(), expected);
            assert_eq!(a.clone() + &b, expected);
            assert_eq!(a + b, expected);
        }

        #[test_case(0, 0, 0; "zero minus zero")]
        #[test_case(BIG_DECIMAL_FRACTIONAL as i128, 0, BIG_DECIMAL_FRACTIONAL as i128; "one minus zero")]
        #[test_case(0, BIG_DECIMAL_FRACTIONAL as i128, -(BIG_DECIMAL_FRACTIONAL as i128); "zero minus one")]
        #[test_case(BIG_DECIMAL_FRACTIONAL as i128, BIG_DECIMAL_FRACTIONAL as i128, 0; "one minus one")]
        #[test_case(1, 0, 1; "10^-18 minus zero")]
        #[test_case(0, 1, -1; "zero minus 10^-18")]
        #[test_case(1, 1, 0; "10^-18 minus 10^-18")]
        fn test_bigdecimal_sub_bigdecimal(a: i128, b: i128, expected: i128) {
            let a = BigDecimal::new(a.into());
            let b = BigDecimal::new(b.into());
            let expected = BigDecimal::new(expected.into());
            assert_eq!(&a - &b, expected);
            assert_eq!(&a - b.clone(), expected);
            assert_eq!(a.clone() - &b, expected);
            assert_eq!(a - b, expected);
        }

        #[test_case(0, 0, 0; "zero times zero")]
        #[test_case(BIG_DECIMAL_FRACTIONAL, 0, 0; "one times zero")]
        #[test_case(0, BIG_DECIMAL_FRACTIONAL, 0; "zero times one")]
        #[test_case(BIG_DECIMAL_FRACTIONAL, BIG_DECIMAL_FRACTIONAL, BIG_DECIMAL_FRACTIONAL; "one times one")]
        #[test_case(1_000_000_000, 1_000_000_000, 1; "10^9 times 10^9")]
        #[test_case(BIG_DECIMAL_FRACTIONAL, BIG_DECIMAL_FRACTIONAL / 2, BIG_DECIMAL_FRACTIONAL / 2; "one times 0.5")]
        fn test_bigdecimal_mul_bigdecimal(a: u128, b: u128, expected: u128) {
            let a = BigDecimal::new(a.into());
            let b = BigDecimal::new(b.into());
            let expected = BigDecimal::new(expected.into());
            println!("{:?}, {:?}", &a * &b, expected);
            assert_eq!(&a * &b, expected);
            assert_eq!(&a * b.clone(), expected);
            assert_eq!(a.clone() * &b, expected);
            assert_eq!(a * b, expected);
        }

        #[test_case(0, 1, 0; "zero over 10^-18")]
        #[test_case(1, 1, BIG_DECIMAL_FRACTIONAL as i128; "10^-18 over 10^-18")]
        #[test_case(BIG_DECIMAL_FRACTIONAL as i128, 2 * BIG_DECIMAL_FRACTIONAL as i128, BIG_DECIMAL_FRACTIONAL as i128 / 2 ; "1 over 2")]
        #[test_case(BIG_DECIMAL_FRACTIONAL as i128, 3 * BIG_DECIMAL_FRACTIONAL as i128, BIG_DECIMAL_FRACTIONAL as i128 / 3 ; "1 over 3")]
        #[test_case(BIG_DECIMAL_FRACTIONAL as i128,  BIG_DECIMAL_FRACTIONAL as i128, BIG_DECIMAL_FRACTIONAL as i128 ; "1 over 1")]
        #[test_case(BIG_DECIMAL_FRACTIONAL as i128,  -(BIG_DECIMAL_FRACTIONAL as i128), -(BIG_DECIMAL_FRACTIONAL as i128) ; "1 over neg 1")]
        fn test_bigdecimal_div_bigdecimal(a: i128, b: i128, expected: i128) {
            let a = BigDecimal::new(a.into());
            let b = BigDecimal::new(b.into());
            let expected = BigDecimal::new(expected.into());
            assert_eq!(&a / &b, expected);
            assert_eq!(&a / b.clone(), expected);
            assert_eq!(a.clone() / &b, expected);
            assert_eq!(a / b, expected);
        }

        #[test_case(0i128 => 0 ; "zero")]
        #[test_case(BIG_DECIMAL_FRACTIONAL as i128 => BIG_DECIMAL_FRACTIONAL ; "one")]
        #[test_case(100 * BIG_DECIMAL_FRACTIONAL as i128 => 10 * BIG_DECIMAL_FRACTIONAL ; "one hundred")]
        #[test_case(2 * BIG_DECIMAL_FRACTIONAL as i128 => 1414213562373095145u128 ; "two")]
        #[test_case(-(BIG_DECIMAL_FRACTIONAL as i128) => panics "Cannot compute the square root of a negative number." ; "negative one")]
        fn test_bigdecimal_sqrt(val: i128) -> u128 {
            bigint_to_u128(&BigDecimal::new(val.into()).sqrt().atomics()).unwrap()
        }
    }
}

/// Calculate how much will be returned from a swap in a constant product pool
fn constant_product_formula(
    offer_reserve: &BigInt,
    ask_reserve: &BigInt,
    offer_amount: Uint128,
    fee: Decimal,
    tax_rate: Decimal,
) -> StdResult<Uint128> {
    println!("constant_product_formula");
    println!(
        "offer_reserve: {}, ask_reserve: {}, offer_amount: {offer_amount}, fee: {fee}",
        offer_reserve, ask_reserve
    );
    let cp = offer_reserve * ask_reserve;
    let return_amount_bigint =
        ask_reserve - cp / (offer_reserve + BigInt::from(offer_amount.u128()));
    let return_amount: Uint128 = bigint_to_u128(&return_amount_bigint)?.into();
    println!("return_amount: {return_amount}");
    let commission_amount = return_amount * fee;
    println!("commission_amount: {commission_amount}");
    let return_amount = (return_amount - commission_amount) * (Decimal::one() - tax_rate);
    println!("return_amount after tax: {return_amount}");
    Ok(return_amount)
}

/// For a constant product pool, calculates how much of one asset we need to
/// swap to the other in order to have the same ratio of assets as the pool, so
/// that we can then provide liquidity and get the most amount of LP tokens.
///
/// Returns `(offer_asset, return_asset): (Asset,Asset)` containing the amount
/// and info of the asset we need to swap, and the asset that will be returned
/// from the swap
pub fn calc_xyk_balancing_swap(
    deps: Deps,
    assets: [Asset; 2],
    reserves: [Uint128; 2],
    fee: Decimal,
    tax_configs: Option<TaxConfigsChecked>,
) -> StdResult<(Asset, Asset)> {
    deps.api.debug("calc_xyk_balancing_swap");
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
    let offer_reserve = &BigInt::from(reserves[offer_idx].u128());
    let ask_reserve = &BigInt::from(reserves[ask_idx].u128());
    let offer_balance = &BigInt::from(assets[offer_idx].amount.u128());
    let ask_balance = &BigInt::from(assets[ask_idx].amount.u128());

    let fee_rate = &BigDecimal::from(fee);

    // Unwrap tax
    let offer_asset_info = &assets[offer_idx].info;
    let tax_rate_decimal = tax_configs
        .map(|tax_configs| {
            tax_configs
                .get(&offer_asset_info.to_string())
                .map(|tax_config| tax_config.tax_rate)
                .unwrap_or(Decimal::zero())
        })
        .unwrap_or(Decimal::zero());
    let tax_rate: &BigDecimal = &tax_rate_decimal.into();

    deps.api.debug("pre calcs");

    // Original formula:
    let two = &BigDecimal::from(2u128);
    let a = ask_reserve + ask_balance;
    let b = two * offer_reserve * (ask_reserve + ask_balance)
        - ((offer_reserve + offer_balance) * ask_reserve * fee_rate);
    let c = offer_reserve * (offer_reserve * ask_balance - offer_balance * ask_reserve);
    let discriminant = &b * &b - (two * two * &a * &c);
    //  We know that for this equation, there is only one positive real solution
    let x = (discriminant.sqrt() - b) / (two * a);

    // New formula including tax:
    // Solve equation to find amount to swap
    // let two = &BigDecimal::from(2u128);
    // let four = two * two;
    // let numerator = offer_reserve * ask_reserve * (fee_rate - fee_rate * tax_rate)
    //     + (offer_balance + offer_reserve) * ask_reserve * fee_rate
    //     - two * offer_reserve * (ask_balance + ask_reserve);
    // deps.api.debug(&format!("numerator: {:?}", numerator));
    // let discriminant = (two * offer_reserve * ask_balance - offer_balance * ask_reserve * fee_rate
    //     + two * offer_reserve * ask_reserve * (BigDecimal::one() - fee_rate)
    //     + offer_reserve * ask_reserve * fee_rate * tax_rate)
    //     .pow(2)
    //     - four
    //         * (ask_balance + ask_reserve + ask_reserve * (fee_rate * tax_rate - tax_rate))
    //         * (offer_reserve.pow(2) * ask_balance - offer_balance * offer_reserve * ask_reserve);
    // deps.api.debug("discriminant: {discriminant}");
    // let denominator = two
    //     * (ask_balance + ask_reserve - ask_reserve * tax_rate + ask_reserve * fee_rate * tax_rate);

    // deps.api.debug("denominator: {denominator}");

    // let x = (numerator + discriminant.sqrt()) / denominator;

    deps.api.debug("x: {x}");

    // Divide by precision to get final result and convert to Uint128
    let offer_amount: Uint128 = Uint128::try_from(x.floor().to_string().as_str())?;
    let offer_asset = Asset {
        amount: offer_amount,
        info: assets[offer_idx].info.clone(),
    };

    // Calculate return amount from swap
    let return_amount = constant_product_formula(
        offer_reserve,
        ask_reserve,
        offer_amount,
        fee,
        tax_rate_decimal,
    )?;
    let return_asset = Asset {
        amount: return_amount,
        info: assets[ask_idx].info.clone(),
    };

    println!("offer_asset: {offer_asset}, return_asset: {return_asset}");

    Ok((offer_asset, return_asset))
}

// #[cfg(test)]
// mod test {
//     use apollo_cw_asset::{Asset, AssetInfo};
//     use cosmwasm_std::{testing::mock_dependencies, Decimal, Uint128};
//     use test_case::test_case;

//     use crate::math::calc_xyk_balancing_swap;

//     /// Assert that two Decimals are almost the same (diff smaller than one
//     /// permille)
//     fn assert_decimal_almost_eq(a: Decimal, b: Decimal) {
//         let diff = if a > b { (a - b) / a } else { (b - a) / b };
//         let max_allowed_diff = Decimal::permille(2);
//         if diff > max_allowed_diff {
//             panic!("Failed assert decimal almost eq for a: {a}, b: {b}. diff: {diff}, max allowed: {max_allowed_diff}");
//         }
//     }

//     // Assert that the ratio of the users assets is the same as the pool after the
//     // swap
//     fn assert_asset_ratios_same_after_swap(
//         offer_reserve: Uint128,
//         ask_reserve: Uint128,
//         offer_balance: Uint128,
//         ask_balance: Uint128,
//         offer_amount: Uint128,
//         return_amount: Uint128,
//     ) {
//         let asset_ratio_after_swap =
//             Decimal::from_ratio(ask_balance + return_amount, offer_balance - offer_amount);
//         let reserve_ratio_after_swap =
//             Decimal::from_ratio(ask_reserve - return_amount, offer_reserve + offer_amount);
//         println!(
//             "asset_ratio_after_swap: {asset_ratio_after_swap}, reserve_ratio_after_swap: {reserve_ratio_after_swap}"
//         );
//         assert_decimal_almost_eq(asset_ratio_after_swap, reserve_ratio_after_swap);
//     }

//     #[test_case(
//         [Uint128::from(1_000_000u128), Uint128::from(2_000_000u128)],
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
//         true,
//         1;
//         "Test 1: 1:1 ratio, double amount of asset 2"
//     )]
//     #[test_case(
//         [Uint128::from(1_000_000u128), Uint128::from(2_000_000u128)],
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(5_000_000_000_000u128)],
//         true,
//         0;
//         "Test 2: 1:5 ratio, double amount of asset 2"
//     )]
//     #[test_case(
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(3_000_000_000_000u128)],
//         true,
//         0;
//         "Test 3: 1:3 pool ratio, 1:1 ratio of assets, but a lot of assets compared to pool (high slipage)"
//     )]
//     #[test_case(
//         [Uint128::from(0u128), Uint128::from(1_000_000_000_000u128)],
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(2_000_000_000_000u128)],
//         true,
//         1;
//         "Test 4: 1:2 pool ratio, 0:1 ratio of assets"
//     )]
//     #[test_case(
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
//         false,
//         1;
//         "Test 5: 1:1 pool ratio, 1:1 ratio of assets"
//     )]
//     #[test_case(
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(0u128)],
//         false,
//         0
//         => panics "No liquidity in pool";
//         "Test 6: 1:0 pool ratio, should fail with correct error"
//     )]
//     #[test_case(
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
//         [Uint128::from(0u128), Uint128::from(1_000_000_000_000u128)],
//         false,
//         0
//         => panics "No liquidity in pool";
//         "Test 7: 0:1 pool ratio, should fail with correct error"
//     )]
//     #[test_case(
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(0u128)],
//         [Uint128::from(1_000_000_000_000u128), Uint128::from(1_000_000_000_000u128)],
//         true,
//         0;
//         "Test 8: 1:1 pool ratio, 1:0 ratio of assets"
//     )]
//     #[test_case(
//         [Uint128::from(0u128), Uint128::from(3564u128)],
//         [Uint128::from(3450765745u128), Uint128::from(12282531965699u128)],
//         true,
//         1;
//         "Test 9: Amount of asset less than one microunit of other asset"
//     )]
//     fn test_calc_xyk_balancing_swap(
//         assets: [Uint128; 2],
//         reserves: [Uint128; 2],
//         should_swap: bool,
//         offer_asset_idx: usize,
//     ) {
//         let assets = [
//             Asset {
//                 amount: assets[0],
//                 info: AssetInfo::native("uatom".to_string()),
//             },
//             Asset {
//                 amount: assets[1],
//                 info: AssetInfo::native("uosmo".to_string()),
//             },
//         ];
//         let offer_asset = assets[offer_asset_idx].clone();
//         let ask_asset = assets[1 - offer_asset_idx].clone();
//         let offer_reserve = reserves[offer_asset_idx];
//         let ask_reserve = reserves[1 - offer_asset_idx];

//         // Same fee for all test cases
//         let fee = Decimal::permille(3);

//         println!("Assets: {assets:?}");
//         println!("Reserves: {reserves:?}");

//         // Calculate swap
//         let (swap_asset, return_asset) =
//             calc_xyk_balancing_swap(mock_dependencies(), assets, reserves, fee, None).unwrap();

//         println!("Swap: {swap_asset:?}, Return: {return_asset:?}");

//         // If ratios are already almost the same, no swap should happen
//         if !should_swap {
//             assert_eq!(swap_asset.amount, Uint128::zero());
//         }

//         // Assert that the correct asset is being offered
//         assert_eq!(swap_asset.info, offer_asset.info);

//         // If the amount returned is zero because the swapped amount is too small
//         // then the following assert will fail, so we just return here
//         if return_asset.amount == Uint128::zero() {
//             return;
//         }

//         // Assert that the asset ratio and the pool ratio are the same after the swap
//         assert_asset_ratios_same_after_swap(
//             offer_reserve,
//             ask_reserve,
//             offer_asset.amount,
//             ask_asset.amount,
//             swap_asset.amount,
//             return_asset.amount,
//         );
//         println!("------------------------------------");
//     }
// }
