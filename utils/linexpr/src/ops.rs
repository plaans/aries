use num_traits::{One, Zero};
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use crate::{LinExpr, VarCoeff};

/// Conversion of an operand (variable, scaled variable, constant, or
/// expression) into a [`LinExpr`].
///
/// This is the linexpr-native counterpart of `Into<LinExpr>`: because the
/// trait is local to this crate, it can be implemented generically for `Num`
/// and `VarCoeff` here, and for the user's own `Var` type by the
/// [`impl_var_ops!`] macro, without tripping the orphan rules.
pub trait IntoLinExpr<Var, Num> {
    /// Converts `self` into the equivalent linear expression.
    fn into_linexpr(self) -> LinExpr<Var, Num>;
}

impl<Var, Num> IntoLinExpr<Var, Num> for Num {
    fn into_linexpr(self) -> LinExpr<Var, Num> {
        LinExpr::from_constant(self)
    }
}

impl<Var, Num: Zero> IntoLinExpr<Var, Num> for VarCoeff<Var, Num> {
    fn into_linexpr(self) -> LinExpr<Var, Num> {
        LinExpr {
            constant: Num::zero(),
            terms: vec![self],
        }
    }
}

impl<Var, Num> IntoLinExpr<Var, Num> for LinExpr<Var, Num> {
    fn into_linexpr(self) -> Self {
        self
    }
}

impl<Var, Num> From<(Var, Num)> for VarCoeff<Var, Num> {
    fn from(term: (Var, Num)) -> Self {
        VarCoeff::new(term.0, term.1)
    }
}

impl<Var, Num: Copy> From<&(Var, Num)> for VarCoeff<Var, Num>
where
    Var: Copy,
{
    fn from(term: &(Var, Num)) -> Self {
        VarCoeff::new(term.0, term.1)
    }
}

impl<Var, Num, Item, I> From<I> for LinExpr<Var, Num>
where
    I: IntoIterator<Item = Item>,
    Item: Into<VarCoeff<Var, Num>>,
    Num: Zero,
{
    fn from(iter: I) -> Self {
        let mut expr = LinExpr::zero();
        for term in iter.into_iter() {
            expr.push(term.into());
        }
        expr
    }
}

impl<Var, Num: Zero> FromIterator<(Var, Num)> for LinExpr<Var, Num> {
    fn from_iter<I: IntoIterator<Item = (Var, Num)>>(iter: I) -> Self {
        let mut expr = LinExpr::zero();
        for (var, coeff) in iter {
            expr.push(VarCoeff::new(var, coeff));
        }
        expr
    }
}

impl<Var, Num: Neg<Output = Num>> Neg for VarCoeff<Var, Num> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        let (var, coeff) = self.into_parts();
        VarCoeff::new(var, -coeff)
    }
}

impl<Var, Num: Zero> Add<VarCoeff<Var, Num>> for VarCoeff<Var, Num> {
    type Output = LinExpr<Var, Num>;

    fn add(self, rhs: VarCoeff<Var, Num>) -> Self::Output {
        LinExpr {
            constant: Num::zero(),
            terms: vec![self, rhs],
        }
    }
}

impl<Var, Num: Zero + Neg<Output = Num>> Sub<VarCoeff<Var, Num>> for VarCoeff<Var, Num> {
    type Output = LinExpr<Var, Num>;

    fn sub(self, rhs: VarCoeff<Var, Num>) -> Self::Output {
        LinExpr {
            constant: Num::zero(),
            terms: vec![self, -rhs],
        }
    }
}

impl<Var, Num: Zero + One> Add<Var> for VarCoeff<Var, Num> {
    type Output = LinExpr<Var, Num>;

    fn add(self, rhs: Var) -> Self::Output {
        self + VarCoeff::from_variable(rhs)
    }
}

impl<Var, Num: Zero + One + Neg<Output = Num>> Sub<Var> for VarCoeff<Var, Num> {
    type Output = LinExpr<Var, Num>;

    fn sub(self, rhs: Var) -> Self::Output {
        self - VarCoeff::from_variable(rhs)
    }
}

impl<Var, Num, T: IntoLinExpr<Var, Num>> Add<T> for LinExpr<Var, Num>
where
    Num: AddAssign,
{
    type Output = LinExpr<Var, Num>;

    fn add(mut self, rhs: T) -> Self::Output {
        let rhs = rhs.into_linexpr();
        self.constant += rhs.constant;
        self.terms.extend(rhs.terms);
        self
    }
}

impl<Var, Num, T: IntoLinExpr<Var, Num>> Sub<T> for LinExpr<Var, Num>
where
    Num: AddAssign + Neg<Output = Num>,
{
    type Output = LinExpr<Var, Num>;

    fn sub(mut self, rhs: T) -> Self::Output {
        let rhs = rhs.into_linexpr();
        self.constant += -rhs.constant;
        for term in rhs.terms {
            self.push(-term);
        }
        self
    }
}

impl<Var, Num, T: IntoLinExpr<Var, Num>> AddAssign<T> for LinExpr<Var, Num>
where
    Num: AddAssign,
{
    fn add_assign(&mut self, rhs: T) {
        let rhs = rhs.into_linexpr();
        self.constant += rhs.constant;
        self.terms.extend(rhs.terms);
    }
}

impl<Var, Num, T: IntoLinExpr<Var, Num>> SubAssign<T> for LinExpr<Var, Num>
where
    Num: AddAssign + Neg<Output = Num>,
{
    fn sub_assign(&mut self, rhs: T) {
        let rhs = rhs.into_linexpr();
        self.constant += -rhs.constant;
        for term in rhs.terms {
            self.push(-term);
        }
    }
}

/// Implements the expression-level operators on a user-defined variable type:
/// `Neg`, `Add<Var>` and `Sub<Var>` on `Var` itself, plus its conversion into a
/// [`LinExpr`].
///
/// Because the signatures of these impls do not mention `Num`, they can exist
/// only once for a given `Var` type: this macro must be invoked at most once
/// per `Var`, with the *primary* numeric type. Use [`impl_scale_ops!`] for
/// additional numeric types.
#[macro_export]
macro_rules! impl_var_ops {
    ($var:ty, $num:ty) => {
        impl ::std::ops::Neg for $var
        where
            $num: $crate::num_traits::One + ::std::ops::Neg<Output = $num>,
        {
            type Output = $crate::VarCoeff<$var, $num>;

            fn neg(self) -> Self::Output {
                -$crate::VarCoeff::from_variable(self)
            }
        }

        impl ::std::ops::Add<$var> for $var
        where
            $num: $crate::num_traits::Zero + $crate::num_traits::One + ::std::ops::AddAssign,
        {
            type Output = $crate::LinExpr<$var, $num>;

            fn add(self, rhs: $var) -> Self::Output {
                $crate::IntoLinExpr::into_linexpr(self) + rhs
            }
        }

        impl ::std::ops::Sub<$var> for $var
        where
            $num: $crate::num_traits::Zero
                + $crate::num_traits::One
                + ::std::ops::AddAssign
                + ::std::ops::Neg<Output = $num>,
        {
            type Output = $crate::LinExpr<$var, $num>;

            fn sub(self, rhs: $var) -> Self::Output {
                $crate::IntoLinExpr::into_linexpr(self) - rhs
            }
        }

        impl $crate::IntoLinExpr<$var, $num> for $var
        where
            $num: $crate::num_traits::Zero + $crate::num_traits::One,
        {
            fn into_linexpr(self) -> $crate::LinExpr<$var, $num> {
                let mut expr = $crate::LinExpr::zero();
                expr.push($crate::VarCoeff::from_variable(self));
                expr
            }
        }
    };
}

/// Implements the scaling operators between a user-defined variable type and a
/// numeric type: `Var * Num`, `Num * Var`, `Var + Num` and `Var - Num`.
///
/// Unlike [`impl_var_ops!`], these impls mention `Num` in their signature, so
/// this macro may be invoked once per `(Var, Num)` pair.
#[macro_export]
macro_rules! impl_scale_ops {
    ($var:ty, $num:ty) => {
        impl ::std::ops::Mul<$num> for $var {
            type Output = $crate::VarCoeff<$var, $num>;

            fn mul(self, rhs: $num) -> Self::Output {
                $crate::VarCoeff::new(self, rhs)
            }
        }

        impl ::std::ops::Mul<$var> for $num {
            type Output = $crate::VarCoeff<$var, $num>;

            fn mul(self, rhs: $var) -> Self::Output {
                $crate::VarCoeff::new(rhs, self)
            }
        }

        impl ::std::ops::Add<$num> for $var
        where
            $num: $crate::num_traits::Zero + $crate::num_traits::One + ::std::ops::AddAssign,
        {
            type Output = $crate::LinExpr<$var, $num>;

            fn add(self, rhs: $num) -> Self::Output {
                let mut expr = $crate::LinExpr::zero();
                expr.push($crate::VarCoeff::from_variable(self));
                expr + rhs
            }
        }

        impl ::std::ops::Sub<$num> for $var
        where
            $num: $crate::num_traits::Zero
                + $crate::num_traits::One
                + ::std::ops::AddAssign
                + ::std::ops::Neg<Output = $num>,
        {
            type Output = $crate::LinExpr<$var, $num>;

            fn sub(self, rhs: $num) -> Self::Output {
                let mut expr = $crate::LinExpr::zero();
                expr.push($crate::VarCoeff::from_variable(self));
                expr - rhs
            }
        }
    };
}

/// Implements all the linear-expression operators for a `(Var, Num)` pair.
///
/// This is the combination of [`impl_var_ops!`] and [`impl_scale_ops!`]: it
/// must be invoked in the crate that owns the `Var` type, at most once per
/// `Var` type. See [`impl_scale_ops!`] for supporting additional numeric
/// types on the same variable type.
///
/// # Example
///
/// ```
/// use aries_linexpr::{ComparisonOp, LinExpr, VarCoeff, impl_linear_ops};
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// pub struct Var(pub usize);
///
/// impl_linear_ops!(Var, f64);
///
/// let x = Var(0);
/// let y = Var(1);
///
/// // 2x + 3y <= 4
/// let constraint = (2.0 * x + 3.0 * y).leq(4.0);
/// assert_eq!(constraint.comp_op(), ComparisonOp::Le);
///
/// // x - y >= 0
/// let constraint = (x - y).geq(0.0);
/// assert_eq!(constraint.comp_op(), ComparisonOp::Ge);
///
/// let scaled: VarCoeff<Var, f64> = 3.0 * x;
/// let expr: LinExpr<Var, f64> = scaled + x;
/// ```
#[macro_export]
macro_rules! impl_linear_ops {
    ($var:ty, $num:ty) => {
        $crate::impl_var_ops!($var, $num);
        $crate::impl_scale_ops!($var, $num);
    };
}
