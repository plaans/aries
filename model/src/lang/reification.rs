use crate::lang::expr::Normalize;
use crate::lang::normal_form::NormalExpr;
use crate::lang::ValidityScope;
use aries_core::literals::Disjunction;
use aries_core::*;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Automatically derived trait for the capabilities of an expression that
/// requires to be inserted into the model (enforced or reified).
///
/// It is typically required that the normal form of any user-facing expression
/// be reifiable.
pub trait ReifiableExpr: ExprInterface + PartialEq + Eq + Hash {}
impl<T: ExprInterface + PartialEq + Eq + Hash> ReifiableExpr for T {}

/// Capabilities of an expression that is read from the model.
pub trait ExprInterface: Any + Debug + Send + Sync + 'static {
    /// Returns a description of the scope in which the expression is valid (i.e. can be evaluated to a value).
    fn validity_scope(&self, presence: &dyn Fn(VarRef) -> Lit) -> ValidityScope;
}

impl ExprInterface for Disjunction {
    fn validity_scope(&self, presence: &dyn Fn(VarRef) -> Lit) -> ValidityScope {
        ValidityScope::new(
            self.literals().iter().map(|l| presence(l.variable())),
            // guard by non optional literal (if one of them is true, the disjunction is defined)
            self.literals()
                .iter()
                .copied()
                .filter(|l| presence(l.variable()) == Lit::TRUE),
        )
    }
}

/// Dynamically typed interface to an expression in the model.
///
/// To transform this dynamically typed expression to its original type,
/// one can use the [`downcast`] function.
pub type Expr = dyn ExprInterface;

/// If the (dynamically typed) referenced `expr` is of type `T`, then casts
/// it and returns it. Returns None if the expr is not of type `T`.
///
/// Source: this implementation is taken from the one of `core::any::downcast_ref` which
/// cannot be directly called as a method on an `&Expr`.
pub fn downcast<T: Any + 'static>(expr: &Expr) -> Option<&T> {
    let tpe = expr.type_id();
    if tpe == TypeId::of::<T>() {
        // CONTEXT: a reference to trait-object is composed of two pointers.
        //  - the first one is a pointer to the original data (here of type T).
        //  - the second one is a pointer to the vtable (here of the trait ExprInterface)
        // In the following, we cast the first pointer to a pointer on T.

        // SAFETY: just checked whether we are pointing to the correct type, and we can rely on
        // that check for memory safety because we have implemented Any for all types; no other
        // impls can exist as they would conflict with our impl.
        unsafe { Some(&*(expr as *const Expr as *const T)) }
    } else {
        None
    }
}

/// A structure to keep track of all binding events that result from reifying and enforcing expressions.
///
/// A correspondence between canonical expression and the literal that they have reified to is maintained.
/// When two literals are bound to the same expression, we record a binding event between the two literals.
///
/// The structure maintains a list of binding events that can be iterated with a [BindingCursor]
#[derive(Default, Clone)]
pub struct Reification {
    /// Associates each canonical atom to a single literal.
    map: HashMap<WrappedExpr, Lit>,
    /// All binding events in chronological order. This is intended to easily process
    /// binding events and detect whether new events have been added.
    binding_events: Vec<(Lit, BindTarget)>,
}

impl Reification {
    fn intern_raw_expr_as(&mut self, be: WrappedExpr, lit: Lit) {
        assert!(!self.map.contains_key(&be));
        self.binding_events.push((lit, BindTarget::Expr(be.value.clone())));
        self.map.insert(be, lit);
    }

    /// If this expression was previously interned, returns the literal it was bound to.
    /// TODO: we should accept a borrowed expression.
    pub fn interned<T: ReifiableExpr>(&mut self, nf: NormalExpr<T>) -> Option<Lit> {
        match nf {
            NormalExpr::Literal(l) => Some(l),
            NormalExpr::Pos(e) => self.map.get(&WrappedExpr::new(e)).copied(),
            NormalExpr::Neg(e) => self.map.get(&WrappedExpr::new(e)).map(|&l| !l),
        }
    }

    /// Interns the user-facing expression, creating a new literal with the `make_lit` closure if the the expression had no preexisting binding.
    pub fn intern_as<T: ReifiableExpr>(&mut self, nf: NormalExpr<T>, lit: Lit) {
        match nf {
            NormalExpr::Literal(_) => panic!("Cannot be interned"),
            NormalExpr::Pos(e) => self.intern_raw_expr_as(WrappedExpr::new(e), lit),
            NormalExpr::Neg(e) => self.intern_raw_expr_as(WrappedExpr::new(e), !lit),
        }
    }

    /// Binds the user-provided expression `expr` to the given `literal`.
    pub fn bind<X: Normalize<T>, T: ReifiableExpr>(&mut self, expr: X, literal: Lit) {
        let nf = expr.normalize();
        match nf {
            NormalExpr::Literal(l) => self.bind_literals(l, literal),
            NormalExpr::Pos(e) => {
                let e = WrappedExpr::new(e);
                if let Some(prev) = self.map.get(&e).copied() {
                    self.bind_literals(prev, literal)
                } else {
                    self.intern_raw_expr_as(e, literal);
                }
            }
            NormalExpr::Neg(e) => {
                let e = WrappedExpr::new(e);
                if let Some(prev) = self.map.get(&e).copied() {
                    self.bind_literals(prev, !literal)
                } else {
                    self.intern_raw_expr_as(e, !literal);
                }
            }
        }
    }

    /// Impose the constraint that the two literals are equal.
    pub fn bind_literals(&mut self, l1: Lit, l2: Lit) {
        self.binding_events.push((l1, BindTarget::Literal(l2)));
    }

    /// Returns the event that this cursors points and advances it to the next one.
    pub fn pop_next_event(&self, cursor: &mut BindingCursor) -> Option<&(Lit, BindTarget)> {
        let ret = self.binding_events.get(cursor.0);
        if ret.is_some() {
            cursor.0 += 1;
        }
        ret
    }
}

/// A wrapper around a dynamically typed expression to allow usage as a key in hash map.
#[derive(Clone)]
struct WrappedExpr {
    /// The expression that we are interested in.
    value: Arc<Expr>,
    /// A lambda function to allow comparing two expressions (the first one being of the same type as `value`
    eq_any: Arc<dyn Fn(&Expr, &Expr) -> bool + Send + Sync>,
    /// A precomputed hash of `value`.
    hash: u64,
}

impl WrappedExpr {
    /// Creates a wrapper around this expression that provides [PartialEq] and [Hash] implementations.
    pub fn new<E: ReifiableExpr>(x: E) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        x.hash(&mut hasher);
        let hash = hasher.finish();
        let value = Arc::new(x);
        let eq_any = Arc::new(|x: &Expr, y: &Expr| {
            let x = downcast::<E>(x).expect("wrong type");
            if let Some(y) = downcast::<E>(y) {
                // x and y are both of type E, we can compare them.
                return x == y;
            }
            false // objects are of different types
        });

        WrappedExpr { value, eq_any, hash }
    }
}

impl Hash for WrappedExpr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.type_id().hash(state);
        self.hash.hash(state);
    }
}

impl PartialEq for WrappedExpr {
    fn eq(&self, other: &Self) -> bool {
        // use the equality lambda built in the object to compare to others.
        self.eq_any.as_ref()(self.value.as_ref(), other.value.as_ref())
    }
}
impl Eq for WrappedExpr {}

/// Target of the binding of literal: either an expression or another literal.
#[derive(Clone, Debug)]
pub enum BindTarget {
    Expr(Arc<Expr>),
    Literal(Lit),
}

/// A cursor into a sequence of bindings.
///
/// The next bidding can be popped with [Reification::pop_next_event].
#[derive(Copy, Clone)]
pub struct BindingCursor(usize);

impl BindingCursor {
    /// Creates a new cursor on the first event.
    pub fn first() -> Self {
        BindingCursor(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::expr::{eq, geq, leq};
    use crate::lang::IVar;

    const A: IVar = IVar::new(VarRef::from_u32(1));
    const B: IVar = IVar::new(VarRef::from_u32(2));
    const C: IVar = IVar::new(VarRef::from_u32(3));

    #[test]
    fn test_reif() {
        let t = Lit::TRUE;
        let f = Lit::FALSE;
        let l1 = leq(A, B + 3);
        let l2 = leq(A, C);
        let e1 = eq(A, B + 3);

        let mut reif = Reification::default();
        reif.bind(l1, t);
        reif.bind(l2, f);
        reif.bind(e1, f);

        assert_eq!(reif.interned(l1.normalize()), Some(t));
        assert_eq!(reif.interned(l2.normalize()), Some(f));
        assert_eq!(reif.interned(e1.normalize()), Some(f));

        // same as l1
        let l1_prime = geq(B + 3, A);
        assert_eq!(reif.interned(l1_prime.normalize()), Some(t));

        // inverse of l1, should return the opposite literal
        assert_eq!(reif.interned((!l1).normalize()), Some(f));
    }
}
