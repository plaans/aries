//! Fundamental types and datastructures of the aries solver (variables, literals, domains & state)
//!
//! This module provide the essential building blocks of the solver:
//!  - [VarRef]: integer variables
//!  - [Lit] (literal): lightweigh boolean statement about the lower or upper bound of a variable
//!  - [Domains](state::Domains): backtrackable datastructures with the current bounds of all integer variables
//!
//!
//! ## Example
//!
//! ```
//! use aries::core::*;
//! use aries::core::state::*;
//! use aries::backtrack::Backtrack;
//! # fn main() {
//! let mut state = Domains::new();
//!
//! // create a new variable with domain [0,10]
//! // the variable is create through the `Domains` datastructure that will keep track of its current domain
//! let x: VarRef = state.new_var(0, 10);
//! assert_eq!(state.lb(x), 0);
//! assert_eq!(state.ub(x), 10);
//!
//! // create a literal representing (x >= 5)
//! let x_ge_5: Lit  = x.geq(5);
//! assert!(!state.entails(x_ge_5), "This literal should no be entailed yet.");
//!
//! // create a backtrack point
//! state.save_state();
//! // restrict the domain of x to [6, 10]
//! state.set_lb(x, 6, Cause::Decision).unwrap();
//! assert_eq!(state.lb(x), 6);
//! assert_eq!(state.ub(x), 10);
//! assert!(state.entails(x_ge_5), "The (x >= 5) literal is now entailed.");
//!
//! // reset the state as it was at the last backtrack point
//! state.restore_last();
//!
//! // the domain of x is back to [0,10]
//! assert_eq!(state.lb(x), 0);
//! assert_eq!(state.ub(x), 10);
//! # }
//! ```

pub use cst::*;
pub use lit::*;
pub use signed_var::*;
pub use variable::*;

mod cst;
mod lit;
pub mod literals;
mod signed_var;
pub mod state;
mod variable;
pub mod views;
