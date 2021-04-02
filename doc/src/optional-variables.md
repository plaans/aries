# Optional Variables

In a CSP, the domain of a discrete variable is composed of a set of integer values which we would encode as a lower and upper bound:
`value(var) ∈ [lb(var), ub(var)]`.

In this classical setting it is impossible to have a variable with `lb(var) > ub(var)`: the domain of the variable would be empty and the contradiction would be raised when setting the lb/ub.


## Domain of optional variables

In aries, we allow a variable to be **optional**. On creation, the model should be provided with:

 - a lower bound `lb`
 - an upper bound `ub`
 - a literal `p` representing whether the variable is present or not.

The initial domain of an optional variable `var` is `⊥ | [lb,ub]` which can be read as:

 - if `var = ⊥`, then the variable is absent from the model
 - otherwise, the variable is present and has a value in `[lb,ub]`

The presence of the variable is controlled by the `p` literal provided at the creation of the variable:
 
 - if `M ⊧ p` then `var ≠ ⊥`. The variable is present an its value must be an integer in `[lb, ub]`.
 - if `M ⊧ !p` then `var = ⊥`, the variable is absent and the bounds are irrelevant to determining its value.

FIXME: here we do not cover the case where the variable underlying `p` is absent: is the literal entailed or not?

## Notations

In the (implicit) context of a particular model, we use the following notation represent the various component of a variable.

- `present(v)` is the literal representing the presence of the variable
- `lb(v)` and `ub(v)` are the lower bound on the integer value of `v` if `v` was to be present.

If `v` is a non-optional variable, then we can assume that `present(v)` returns the tautology literal.



## Maintaining consistency of the model with optional variables

If the integer part of the domain of an optional variable becomes empty (`lb(v) > ub(v)`) then the only possible binding for `v` is the absent value ⊥.
Because the model must always notify of an inconsistency, it should be checked whether this remains a possible value.

Recall that `v = ⊥ ⇒ !present(v)`. Since the ⊥ is only potential value for `v`, we can immediately enforce that the `present(v)` literal is false.

The result of setting a bound of an optional variable `v` will be :

- OK, if after the update `lb(v) <= ub(v)`
- if after the update `lb(v) > ub(v)`:
  - OK, if `!present(v)` is consistent with the model,
  - ERR, if `!present(v)` is inconsistent with the model.

An important consequence of this procedure is that modifying the bounds of an optional variable might result in updates on the domain of other variables, those involved in the definition of its presence literal.

Below is some high level pseudo-code for updating the bounds of an optional variable:

```text
set_lower_bound(var, value):
  lb(var) = value
  if lb(var) > ub(var):
    return set(!present(var))
  else
    return OK

set_upper_bound(var, value):
  ub(var) = value
  if lb(var) > ub(var):
    return set(!present(var))
  else
    return OK
```



## Meaning of upper and lower bounds of optional variables

Regardless of whether a variable is present or absent, the system will keep track of the `lb(v)` and `ub(v)`, the lower and upper bounds of any variable `v`.

**Integer domain**: If a variable `v` is present (I.e. ≠ ⊥), then its value must be an integer contained in the `[lb(v), ub(v)]` interval.
`[lb(v), ub(v)]` is referred to as the integer domain of `v`.

Note that the integer domain of a variable is meaningless if the variable is absent. In this case and in this case only the integer domain might be empty.


