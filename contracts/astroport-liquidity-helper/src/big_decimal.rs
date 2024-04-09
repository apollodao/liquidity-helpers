
use std::ops::{Add, Div, Mul, Sub};

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

        while y < x {
            x = y.clone();
            y = (x.clone() + self.clone() / x.clone()) / BigDecimal::from(2u128);
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

        // BigDecimal is a fixed-point number with BIG_DECIMAL_FRACTIONAL decimal
        // places. x^y = (numerator / denominator)^y = numerator^y /
        // denominator^y     = (numerator^y / denominator^(y-1)) /
        // denominator which means we represent the new number as
        // new_numerator = numerator^y / denominator^(y-1),
        // and new_denominator = denominator.
        let value: BigInt = self.0.pow(exp) / BIG_DECIMAL_FRACTIONAL.pow(exp - 1);

        Self(value)
    }

    pub fn floor(&self) -> BigInt {
        self.0.clone() / BIG_DECIMAL_FRACTIONAL
    }
}

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

impl<'a> Add<&'a BigInt> for BigDecimal {
    type Output = BigDecimal;

    fn add(self, rhs: &'a BigInt) -> Self::Output {
        self + rhs.clone()
    }
}

impl<'a> Add<BigDecimal> for &'a BigInt {
    type Output = BigDecimal;

    fn add(self, rhs: BigDecimal) -> Self::Output {
        rhs + self.clone()
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

impl<'a> Sub<BigInt> for &'a BigDecimal {
    type Output = BigDecimal;

    fn sub(self, rhs: BigInt) -> Self::Output {
        BigDecimal(self.0.clone() - rhs * BIG_DECIMAL_FRACTIONAL)
    }
}

impl<'a> Sub<&'a BigInt> for BigDecimal {
    type Output = BigDecimal;

    fn sub(self, rhs: &'a BigInt) -> Self::Output {
        BigDecimal(self.0 - rhs.clone() * BIG_DECIMAL_FRACTIONAL)
    }
}

impl<'a> Sub<BigDecimal> for &'a BigInt {
    type Output = BigDecimal;

    fn sub(self, rhs: BigDecimal) -> Self::Output {
        BigDecimal(self.clone() * BIG_DECIMAL_FRACTIONAL - rhs.0)
    }
}

impl<'a> Sub<&'a BigDecimal> for BigInt {
    type Output = BigDecimal;

    fn sub(self, rhs: &'a BigDecimal) -> Self::Output {
        BigDecimal(self * BIG_DECIMAL_FRACTIONAL - rhs.0.clone())
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
    #[test_case(2 * BIG_DECIMAL_FRACTIONAL as i128 => 1414213562373095048u128 ; "two")]
    #[test_case(-(BIG_DECIMAL_FRACTIONAL as i128) => panics "Cannot compute the square root of a negative number." ; "negative one")]
    fn test_bigdecimal_sqrt(val: i128) -> u128 {
        bigint_to_u128(&BigDecimal::new(val.into()).sqrt().atomics()).unwrap()
    }

    #[test_case(0, 0, 0; "zero plus zero")]
    #[test_case(BIG_DECIMAL_FRACTIONAL, 0, BIG_DECIMAL_FRACTIONAL; "one plus zero")]
    #[test_case(0, 1, BIG_DECIMAL_FRACTIONAL; "zero plus one")]
    #[test_case(BIG_DECIMAL_FRACTIONAL, 1, 2 * BIG_DECIMAL_FRACTIONAL; "one plus one")]
    fn test_bigdecimal_add_bigint(a: u128, b: u128, expected: u128) {
        let a = BigDecimal::new(a.into());
        let b = BigInt::from(b);
        let expected = BigDecimal::new(expected.into());
        assert_eq!(&a + &b, expected);
        assert_eq!(&a + b.clone(), expected);
        assert_eq!(a.clone() + &b, expected);
        assert_eq!(a + b, expected);
    }

    #[test_case(0, 0, 0; "zero minus zero")]
    #[test_case(BIG_DECIMAL_FRACTIONAL as i128, 0, BIG_DECIMAL_FRACTIONAL as i128; "one minus zero")]
    #[test_case(0, 1, -(BIG_DECIMAL_FRACTIONAL as i128); "zero minus one")]
    #[test_case(BIG_DECIMAL_FRACTIONAL as i128, 1, 0; "one minus one")]
    fn test_bigdecimal_sub_bigint(a: i128, b: i128, expected: i128) {
        let a = BigDecimal::new(a.into());
        let b = BigInt::from(b);
        let expected = BigDecimal::new(expected.into());
        assert_eq!(&a - &b, expected);
        assert_eq!(&a - b.clone(), expected);
        assert_eq!(a.clone() - &b, expected);
        assert_eq!(a - b, expected);
    }

    #[test_case(0, 0, 0; "zero times zero")]
    #[test_case(BIG_DECIMAL_FRACTIONAL as i128, 0, 0; "one times zero")]
    #[test_case(0, 1, 0; "zero times one")]
    #[test_case(BIG_DECIMAL_FRACTIONAL as i128, 1, BIG_DECIMAL_FRACTIONAL as i128; "one times one")]
    #[test_case(BIG_DECIMAL_FRACTIONAL as i128, -1, -(BIG_DECIMAL_FRACTIONAL as i128); "one times neg one")]
    #[test_case(1, 1, 1; "10^-18 times one")]
    #[test_case(-(BIG_DECIMAL_FRACTIONAL as i128), 1, -(BIG_DECIMAL_FRACTIONAL as i128); "neg one times one")]
    fn test_bigdecimal_mul_bigint(a: i128, b: i128, expected: i128) {
        let a = BigDecimal::new(a.into());
        let b = BigInt::from(b);
        let expected = BigDecimal::new(expected.into());
        assert_eq!(&a * &b, expected);
        assert_eq!(&a * b.clone(), expected);
        assert_eq!(a.clone() * &b, expected);
        assert_eq!(a * b, expected);
    }
}
