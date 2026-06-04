//! Arithmetic and conversion operations for numeric types.
//!
//! This module contains all implementations of arithmetic operations (`Add`, `Sub`, `Mul`, `Neg`)
//! and conversions (`From`, `TryFrom`) between the numeric types in the language module.

use crate::core::IntoIntCst;
use crate::core::{IntCst, SignedVar, Var};
use crate::lang::ConversionError;
use crate::lang::int::IAtom;
use crate::lang::linear::{LinSum, LinTerm, ScaledVar};
use crate::{transitive_conversion, transitive_conversions};

/* ========================================================================== */
/*                              IAtom Operations                               */
/* ========================================================================== */

impl From<Var> for IAtom {
    fn from(v: Var) -> Self {
        IAtom::new(v, 0)
    }
}

impl<T: IntoIntCst> From<T> for IAtom {
    fn from(i: T) -> Self {
        IAtom::new(Var::ZERO, i.into_int_cst())
    }
}

impl TryFrom<IAtom> for Var {
    type Error = ConversionError;

    fn try_from(value: IAtom) -> Result<Self, Self::Error> {
        if value.shift == 0 {
            Ok(value.var)
        } else {
            Err(ConversionError::NotPure)
        }
    }
}

impl TryFrom<IAtom> for IntCst {
    type Error = ConversionError;

    fn try_from(value: IAtom) -> Result<Self, Self::Error> {
        match value.var {
            Var::ZERO => Ok(value.shift),
            _ => Err(ConversionError::NotConstant),
        }
    }
}

impl<T: IntoIntCst> std::ops::Add<T> for IAtom {
    type Output = IAtom;

    fn add(self, rhs: T) -> Self::Output {
        IAtom::new(self.var, self.shift + rhs.into_int_cst())
    }
}

impl<T: IntoIntCst> std::ops::Add<T> for Var {
    type Output = IAtom;

    fn add(self, rhs: T) -> Self::Output {
        IAtom::new(self, rhs.into_int_cst())
    }
}

impl<T: IntoIntCst> std::ops::Sub<T> for IAtom {
    type Output = IAtom;

    fn sub(self, rhs: T) -> Self::Output {
        IAtom::new(self.var, self.shift - rhs.into_int_cst())
    }
}

impl<T: IntoIntCst> std::ops::Sub<T> for Var {
    type Output = IAtom;

    fn sub(self, rhs: T) -> Self::Output {
        IAtom::new(self, -rhs.into_int_cst())
    }
}

// IAtom * constant -> LinTerm (since IAtom = Var + c1, and (Var + c1) * c2 = c2*Var + c2*c1 = LinTerm)
impl<T: IntoIntCst> std::ops::Mul<T> for IAtom {
    type Output = LinTerm;

    fn mul(self, rhs: T) -> Self::Output {
        let cst = rhs.into_int_cst();
        LinTerm::new(ScaledVar::new(self.var, cst), self.shift * cst)
    }
}

// Reverse: IntCst * IAtom
impl std::ops::Mul<IAtom> for IntCst {
    type Output = LinTerm;

    fn mul(self, rhs: IAtom) -> Self::Output {
        rhs * self
    }
}

/* ========================================================================== */
/*                            ScaledVar Operations                            */
/* ========================================================================== */

impl From<SignedVar> for ScaledVar {
    fn from(value: SignedVar) -> Self {
        Self {
            var: value.variable(),
            factor: value.sign(),
        }
    }
}

// Direct conversion from any type that can be converted to IntCst to ScaledVar (constant term)
impl<T: IntoIntCst> From<T> for ScaledVar {
    fn from(cst: T) -> Self {
        ScaledVar::new(Var::ZERO, cst.into_int_cst())
    }
}

impl<T: IntoIntCst> std::ops::Mul<T> for Var {
    type Output = ScaledVar;

    fn mul(self, rhs: T) -> Self::Output {
        ScaledVar::new(self, rhs.into_int_cst())
    }
}

impl std::ops::Mul<Var> for IntCst {
    type Output = ScaledVar;

    fn mul(self, rhs: Var) -> Self::Output {
        ScaledVar::new(rhs, self)
    }
}

impl<T: IntoIntCst> std::ops::Add<T> for SignedVar {
    type Output = LinTerm;

    fn add(self, rhs: T) -> Self::Output {
        self.sign() * self.variable() + rhs
    }
}

impl<T: IntoIntCst> std::ops::Sub<T> for SignedVar {
    type Output = LinTerm;

    fn sub(self, rhs: T) -> Self::Output {
        self.sign() * self.variable() - rhs
    }
}

impl<T: IntoIntCst> std::ops::Add<T> for ScaledVar {
    type Output = LinTerm;

    fn add(self, rhs: T) -> Self::Output {
        LinTerm::new(self, rhs.into_int_cst())
    }
}

impl<T: IntoIntCst> std::ops::Sub<T> for ScaledVar {
    type Output = LinTerm;

    fn sub(self, rhs: T) -> Self::Output {
        LinTerm::new(self, -rhs.into_int_cst())
    }
}

// ScaledVar * constant -> ScaledVar
impl<T: IntoIntCst> std::ops::Mul<T> for ScaledVar {
    type Output = ScaledVar;

    fn mul(self, rhs: T) -> Self::Output {
        ScaledVar::new(self.var, self.factor * rhs.into_int_cst())
    }
}

// SignedVar * constant -> ScaledVar
impl<T: IntoIntCst> std::ops::Mul<T> for SignedVar {
    type Output = ScaledVar;

    fn mul(self, rhs: T) -> Self::Output {
        ScaledVar::new(self.variable(), self.sign() * rhs.into_int_cst())
    }
}

impl std::ops::Add<SignedVar> for IntCst {
    type Output = LinTerm;

    fn add(self, rhs: SignedVar) -> Self::Output {
        rhs + self
    }
}

impl std::ops::Add<ScaledVar> for IntCst {
    type Output = LinTerm;

    fn add(self, rhs: ScaledVar) -> Self::Output {
        rhs + self
    }
}

// Reverse operations for ScaledVar multiplication
impl std::ops::Mul<ScaledVar> for IntCst {
    type Output = ScaledVar;

    fn mul(self, rhs: ScaledVar) -> Self::Output {
        rhs * self
    }
}

// Reverse operations for SignedVar multiplication
impl std::ops::Mul<SignedVar> for IntCst {
    type Output = ScaledVar;

    fn mul(self, rhs: SignedVar) -> Self::Output {
        rhs * self
    }
}

impl std::ops::Neg for ScaledVar {
    type Output = ScaledVar;

    fn neg(self) -> Self::Output {
        ScaledVar::new(self.var, -self.factor)
    }
}

impl std::ops::Neg for &ScaledVar {
    type Output = ScaledVar;

    fn neg(self) -> Self::Output {
        ScaledVar::new(self.var, -self.factor)
    }
}

/* ========================================================================== */
/*                              LinTerm Operations                             */
/* ========================================================================== */

// Conversions into LinTerm from any type that can be converted to IntCst
impl<T: IntoIntCst> From<T> for LinTerm {
    fn from(value: T) -> Self {
        LinTerm::int_cst(value.into_int_cst())
    }
}

impl From<IAtom> for LinTerm {
    fn from(value: IAtom) -> Self {
        Self {
            scaled_var: ScaledVar::new(value.var, 1),
            constant: value.shift,
        }
    }
}

impl From<ScaledVar> for LinTerm {
    fn from(value: ScaledVar) -> Self {
        Self::new(value, 0)
    }
}

impl TryFrom<LinTerm> for ScaledVar {
    type Error = ConversionError;

    fn try_from(value: LinTerm) -> Result<Self, Self::Error> {
        if value.constant == 0 {
            Ok(value.scaled_var)
        } else {
            Err(ConversionError::NotPure)
        }
    }
}

impl TryFrom<LinTerm> for IntCst {
    type Error = ConversionError;

    fn try_from(value: LinTerm) -> Result<Self, Self::Error> {
        if value.scaled_var.is_zero() {
            Ok(value.constant)
        } else {
            Err(ConversionError::NotConstant)
        }
    }
}

impl std::ops::Neg for LinTerm {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.scaled_var, -self.constant)
    }
}

impl<T: Into<LinTerm>> std::ops::Add<T> for LinTerm {
    type Output = LinSum;

    fn add(self, rhs: T) -> Self::Output {
        LinSum::from(self) + rhs.into()
    }
}

impl std::ops::Sub<LinTerm> for LinTerm {
    type Output = LinSum;

    fn sub(self, rhs: Self) -> Self::Output {
        LinSum::from(self) - rhs
    }
}

// LinTerm * constant -> LinSum
impl<T: IntoIntCst> std::ops::Mul<T> for LinTerm {
    type Output = LinSum;

    fn mul(self, rhs: T) -> Self::Output {
        LinSum::from(self) * rhs
    }
}

// Reverse: IntCst * LinTerm -> LinSum
impl std::ops::Mul<LinTerm> for IntCst {
    type Output = LinSum;

    fn mul(self, rhs: LinTerm) -> Self::Output {
        rhs * self
    }
}

/* ========================================================================== */
/*                              LinSum Operations                              */
/* ========================================================================== */

impl From<LinTerm> for LinSum {
    fn from(value: LinTerm) -> Self {
        if value.scaled_var.is_zero() {
            LinSum::cst(value.constant)
        } else {
            Self::new(value.constant, [value.scaled_var])
        }
    }
}

impl TryFrom<LinSum> for LinTerm {
    type Error = ConversionError;

    fn try_from(value: LinSum) -> Result<Self, Self::Error> {
        let cst = value.constant();
        match value.terms_slice() {
            [] => Ok(LinTerm::int_cst(cst)),
            [sv] => Ok(LinTerm::new(*sv, cst)),
            _ => Err(ConversionError::NotVariable),
        }
    }
}

impl<T: Into<Self>> std::ops::AddAssign<T> for LinSum {
    fn add_assign(&mut self, rhs: T) {
        self.add_assign_impl(rhs.into());
    }
}

impl<T: Into<Self>> std::ops::Add<T> for LinSum {
    type Output = Self;

    fn add(mut self, rhs: T) -> Self::Output {
        self += rhs;
        self
    }
}

impl std::ops::Neg for LinSum {
    type Output = LinSum;

    fn neg(mut self) -> Self::Output {
        self *= -1;
        self
    }
}

impl<T: Into<Self>> std::ops::SubAssign<T> for LinSum {
    fn sub_assign(&mut self, rhs: T) {
        *self += -rhs.into();
    }
}

impl<T: Into<Self>> std::ops::Sub<T> for LinSum {
    type Output = Self;

    fn sub(mut self, rhs: T) -> Self::Output {
        self -= rhs;
        self
    }
}

impl<T: IntoIntCst> std::ops::MulAssign<T> for LinSum {
    fn mul_assign(&mut self, rhs: T) {
        self.mul_assign_impl(rhs.into_int_cst());
    }
}

impl<T: IntoIntCst> std::ops::Mul<T> for LinSum {
    type Output = Self;

    fn mul(mut self, rhs: T) -> Self::Output {
        self *= rhs;
        self
    }
}

impl std::ops::Mul<LinSum> for IntCst {
    type Output = LinSum;

    fn mul(self, mut rhs: LinSum) -> Self::Output {
        rhs *= self;
        rhs
    }
}

/* ========================================================================== */
/*                            Transitive Conversions                           */
/* ========================================================================== */

// Var -> ScaledVar -> LinTerm -> LinSum
transitive_conversion!(LinTerm, ScaledVar, SignedVar);
transitive_conversion!(LinTerm, IAtom, Var);
transitive_conversion!(LinSum, LinTerm, Var);
transitive_conversion!(LinSum, LinTerm, SignedVar);
transitive_conversion!(LinSum, LinTerm, IAtom);
transitive_conversions!(LinSum, LinTerm, IntCst);
transitive_conversions!(LinSum, LinTerm, ScaledVar);

// Note: We already have direct conversions for most paths:
// - ScaledVar, SignedVar, Var are handled via existing From impls
// - LinTerm and LinSum conversions are handled via the main transitive conversions above

/* ========================================================================== */
/*                          Additional Reverse Operations                        */
/* ========================================================================== */

// Reverse operations for IntCst + Var
impl std::ops::Add<Var> for IntCst {
    type Output = IAtom;

    fn add(self, rhs: Var) -> Self::Output {
        rhs + self
    }
}

// Reverse operations for IntCst - Var
impl std::ops::Sub<Var> for IntCst {
    type Output = IAtom;

    fn sub(self, rhs: Var) -> Self::Output {
        IAtom::new(rhs, -self)
    }
}

// Reverse operations for IntCst + IAtom
impl std::ops::Add<IAtom> for IntCst {
    type Output = IAtom;

    fn add(self, rhs: IAtom) -> Self::Output {
        rhs + self
    }
}

// Reverse operations for IntCst - IAtom
impl std::ops::Sub<IAtom> for IntCst {
    type Output = IAtom;

    fn sub(self, rhs: IAtom) -> Self::Output {
        IAtom::new(rhs.var, self - rhs.shift)
    }
}

// Reverse operations for IntCst + LinTerm
impl std::ops::Add<LinTerm> for IntCst {
    type Output = LinSum;

    fn add(self, rhs: LinTerm) -> Self::Output {
        rhs + self
    }
}

// Reverse operations for IntCst - LinTerm
impl std::ops::Sub<LinTerm> for IntCst {
    type Output = LinSum;

    fn sub(self, rhs: LinTerm) -> Self::Output {
        LinSum::cst(self) - rhs
    }
}

// Reverse operations for IntCst + LinSum
impl std::ops::Add<LinSum> for IntCst {
    type Output = LinSum;

    fn add(self, rhs: LinSum) -> Self::Output {
        rhs + self
    }
}

// Reverse operations for IntCst - LinSum
impl std::ops::Sub<LinSum> for IntCst {
    type Output = LinSum;

    fn sub(self, rhs: LinSum) -> Self::Output {
        LinSum::cst(self) - rhs
    }
}

// From<Var> for ScaledVar via SignedVar
transitive_conversion!(ScaledVar, SignedVar, Var);

// Note: We have direct From<IntCst> for LinTerm, so we don't need transitive_conversion!(LinTerm, ScaledVar, IntCst)
// But we do need the transitive conversions that were already defined above

/* ========================================================================== */
/*                                 Unit Tests                                 */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Var;

    /// Test that all sensible arithmetic operations compile between the main types.
    /// This test only checks that the code compiles, not that it produces correct results.
    #[test]
    fn test_arithmetic_operations_compile() {
        // Create some test variables
        let x: Var = Var::from_u32(1);
        let _y: Var = Var::from_u32(2);

        // Test IntCst operations
        let _: IntCst = 5;
        let _: IAtom = 10.into();
        let _: ScaledVar = 15.into();
        let _: LinTerm = 20.into();
        let _: LinSum = 25.into();

        // Test Var operations
        let _: IAtom = x.into();
        let _: ScaledVar = x.into();
        let _: LinTerm = x.into();
        let _: LinSum = x.into();

        // Test IAtom operations
        let a: IAtom = x.into();
        let _: IAtom = a + 5;
        let _: IAtom = a - 3;
        let _: IAtom = 7 + x;
        let _: IAtom = 10 - x;
        let _: IAtom = 5 + a;
        let _: IAtom = 10 - a;
        let _: LinTerm = a * 3; // IAtom * IntCst -> LinTerm
        let _: LinTerm = 3 * a; // IntCst * IAtom -> LinTerm

        // Test ScaledVar operations
        let sv: ScaledVar = x * 3;
        let _: ScaledVar = x * 5;
        let _: ScaledVar = 5 * x;
        let _: ScaledVar = -sv;
        let _: ScaledVar = -&sv;
        let _: ScaledVar = sv * 2; // ScaledVar * IntCst -> ScaledVar
        let _: ScaledVar = 2 * sv; // IntCst * ScaledVar -> ScaledVar
        let _: LinTerm = sv + 5;
        let _: LinTerm = sv - 3;
        let _: LinTerm = 10 + sv;

        // Test SignedVar operations
        let sv2: SignedVar = SignedVar::plus(x);
        let _: ScaledVar = sv2 * 3; // SignedVar * IntCst -> ScaledVar
        let _: ScaledVar = 3 * sv2; // IntCst * SignedVar -> ScaledVar
        let _: LinTerm = sv2 + 5;
        let _: LinTerm = sv2 - 3;
        let _: LinTerm = 10 + sv2;

        // Test LinTerm operations
        let lt: LinTerm = sv + 5;
        let _: LinTerm = ScaledVar::new(x, 2) + 3;
        let _: LinTerm = -lt;
        let _: LinSum = 5 + lt;
        let _: LinSum = 10 - lt;
        let _: LinSum = lt * 2; // LinTerm * IntCst -> LinSum
        let _: LinSum = 2 * lt; // IntCst * LinTerm -> LinSum
        let _: LinSum = lt + lt;
        let _: LinSum = LinSum::from(lt) - 5;
        let _: LinSum = 5 - lt;

        // Test LinSum operations
        let ls: LinSum = lt + lt;
        let _: LinSum = ls.clone() + ls.clone();
        let _: LinSum = ls.clone() - ls.clone();
        let _: LinSum = ls.clone() + 5;
        let _: LinSum = 5 + ls.clone();
        let _: LinSum = ls.clone() - 3;
        let _: LinSum = 10 - ls.clone();
        let _: LinSum = ls.clone() * 2;
        let _: LinSum = 3 * ls.clone();
        let _: LinSum = ls.clone() * 10;
        let _: LinSum = 10 * ls.clone();
        let _: LinSum = -ls.clone();
        let _: LinSum = ls.clone() + x;
        let _: LinSum = 5 - ls.clone();

        // Test conversions
        let _: ScaledVar = x.into();
        let _: LinTerm = x.into();
        let _: LinTerm = sv.into();
        let _: LinTerm = sv2.into();
        let _: LinTerm = a.into();
        let _: LinSum = x.into();
        let _: LinSum = sv.into();
        let _: LinSum = sv2.into();
        let _: LinSum = a.into();
        let _: LinSum = lt.into();
        let _: LinSum = ls.clone();

        // Test reverse operations
        let _: ScaledVar = x * 5;
        let _: ScaledVar = 5 * x;
        let _: IAtom = x + 5;
        let _: IAtom = 5 + x;
        let _: IAtom = x - 5;

        // Test TryFrom conversions
        let _: Result<Var, _> = TryFrom::try_from(a);
        let _: Result<IntCst, _> = TryFrom::try_from(a);
        let _: Result<ScaledVar, _> = TryFrom::try_from(lt);
        let _: Result<IntCst, _> = TryFrom::try_from(lt);
        let _: Result<LinTerm, _> = TryFrom::try_from(ls.clone());

        // Test with different integer types (usize, i32, etc.)
        // Note: We can't do 10usize + x directly because usize doesn't implement Add<Var>
        // But we can use IntoIntCst types in operations where the local type is on the left
        let _: IAtom = x + 10usize; // Var + usize works because Var implements Add<T: IntoIntCst>
        let _: ScaledVar = x * 10usize; // Var * usize works because Var implements Mul<T: IntoIntCst>
        let _: LinTerm = sv + 10usize; // ScaledVar + usize works
        let _: LinSum = ls.clone() * 10usize; // LinSum * usize works
    }
}
