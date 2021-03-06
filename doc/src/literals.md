# Literals


A literal is a boolean assertion on the bounds of a variable.

Given a variable `var` and an integer value `val, a literal is either:

 - `var <= val`, or
 - `var > val`

A literal can be negated: `!(var <= val) = (var > val)`.




## Tautology and Contradiction

The `true` and `false` values are provided as assertions on the zero variable:

 - `Lit::TRUE =  (VarRef::ZERO <= 0)`
 - `Lit::FALSE = !Lit::TRUE = (VarRef::ZERO > 0)`

Because `VarRef::ZERO` is universally assumed to have a single possible value of `0`, it can be assumed that `Lit::TRUE` and `Lit::FALSE` can respectively represent a tautology and contradiction regardless of the context in which they appear.