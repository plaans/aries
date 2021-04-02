# STN Model

## Background

Let `V` be a set of variables with integer domains.[^STN/difference logic can also support other kinds of domains such as real or rationals.]
An atom in difference logic is an expression of the form `v2 - v1 <= X` where `v1 ∈ V`, `v2 ∈ V` and `X ∈ Integers`.

The negation of this atom, is another atom in the same language: 
```text
!(v2 - v1 ≤ X)
   = v2 - v1 > X
   = v1 - v2 < -X
   = v1 - v2 <= -X + 1
```

A conjunction of such atoms form a Simple Temporal Network (STN) in the AI community.
It is a also as a formula in difference logic (sometimes referred to as separation logic) in the automated reasoning community.

### Bound consistency 

If know the expression `v2 ≤ v1 + X` (or equivalently `v2 - X ≤ v1`) to be true we can infer:

 - `lb(v1) ≥ lb(v2) - X`
 - `ub(v2) ≤ ub(v1) + X`

 ### Bound propagation

If a new lower bound `L` is learnt for variable `orig`, we can propagate this information to other variables that appear in the same atom as `orig`:

```text
propagate_lower_bound_update(orig)
    for all (orig ≤ target + X) ∈ Facts:
        lb(target) ← max(lb(target), lb(orig) - X)
```
This update might result in other lower bound updates which can themselves be propagated.


We can have a similar procedure to update upper bounds.
```text
propagate_upper_bound_update(orig):
    for all (target ≤ orig + X) ∈ Facts:
        ub(target) ← min(ub(target), ub(orig) + X)
```

### Distance graph view

In an STN, the `v2 - v1 ≤ X` expression is typically represented by a directed edge:

`v1 ------ X -------> v2`

Note that propagation of :

- upper bounds follow the forward direction (from v1 to v2)
- lower bounds follow the backward direction (from v2 to v1)



## Extension for optional variables

Consider we have the atom `(a ≤ b + X)` where `a` and `b` are optional integer variables.
An arithmetic expression like this one only makes sense if both `a` and `b` have an integer value which is equivalent to saying that they are both *present* in the final model (a ≠ ⊥ ∧ b ≠ ⊥).

Thus an expression `(a ≤ b + X)` has an *optional boolean* type: its value can be *true*, *false* or *absent* (⊥) depending on the values of `a` and `b`:

| Value | Equivalent expression on `a` and `b`              |
|:-----:|:-------------------------------------------------:|
| true  | present(a) ∧ present(b) ∧ value(a) ≤ value(b) + X |
| false | present(a) ∧ present(b) ∧ value(a) > value(b) + X |
|   ⊥   | !present(a) ∨ !present(b)                         |


The question that we want to answer is *In which condition can we propagate updates of the integer domain of b to the integer domain of a?*
Propagation from b to a would be a forward/ub propagation: learning a new value for `ub(b)`the update:
`ub(a) = max { ub(a), ub(b) + X }`.

Let us first observe that if we know that `(a ≤ b + X)` must hold then we can immediately propagate it. This represents the trivial case where both variables are necessarily present.

Remark now that if `a` is absent then the `ub(a)`bound is meaningless and can be modified arbitrarily.

Noting as `l` the literal representing the truth value of the expression `a ≤ b + X`, we are allowed to forward propagate whenever we know that `l` is true or that `present(a)` is false. This might seem like a use less fact. Sure if know that `l` is true we want to propagate, however there is no use in updating the upper bound of `a` if know it will be absent.

The interesting part is when observing that there are many situations where we know that `present(a) ⇒ l` (or equivalently `!present(a) ∨ l`).





```






```

# Outdated 


 - `holds`:  `(present(a) ∧ present(b)) ⇒ l`. Equivalent to `!present(a) ∨ !present(b) ∨ l`
 - `a_entails_b`: `present(a) ⇒ present(b)`. Equivalent to `!present(a) ∨ present(b)`
 - `b_entails_a`: `present(b) ⇒ present(a)`. Equivalent to `!present(b) ∨ present(a)`

The question that we want to answer is *In which condition can we propagate updates of the integer domain of a to the integer domain of b?*

**Proposition:** If `present(b)` entails `present(a)` and `holds = true` then updates to the bounds of `a` can be propagated to the bounds of `b`.

**Proof:**

- if `present(b)` is true then `present(a)` is true. Consequently `l` is true. Both variables have an integer domain for which the relation should hold.
- if `present(b)` is false. Then `b = ⊥` and its integer domain is irrelevant, thus any modification to this domain can be done without consequences.


**Propagation direction**: An upper bound propagation is a propagation from `b` to `a`. A lower bound propagation is a propagation from `a` to `b`.

When registering new reified atom `l = (a ≤ b + X)` we require to literals

 - `ub_trigger` which if entailed allows to propagate upper bounds from `b` to `a`.
 - `lb_trigger` which if entailed allows to propagate lower bounds from `a` to `b`.

**Soundness condition:**

 - it is sound to propagate upper bound updates of `b` to `a` if `ub_trigger ⇒ holds ∧ a_entails_b` 
 - it is sound to propagate lower bound updates of `a` to `b` if `lb_trigger ⇒ holds ∧ b_entails_a` 

 holds & a_entails_b
    = (!pa | !pb | l) & (!pa | pb)
    = (!pa | l)&

### The case of non-optional variables

In the case of non-optional variables we would have the following values:

 - `holds = l`
 - `a_entails_b = true`
 - `b_entails_a = true`
 - `ub_trigger = l`
 - `lb_trigger = l`

Which is equivalent to the standard semantics: if `l = true` then propagation should occur in both directions.



## Theory propagation


COnsider a path of length K: `a_i <== ... <== a_1 <== root <---- b_1 ... <--  b_j` where the double/simple are respectively ub/lb active.

Consequences of the edges being active:
 - `present(a_1) ⇒ present(root)` 
 - `present(b_1) ⇒ present(root)`
 - `present(a_i) ⇒ present(a_{i-1})`
 - `present(b_j) ⇒ present(b_{j-1})`

If we have an inactive edge `a_i --- K' ---> b_j` such that K + K' < 0 then we can infer that its upper bound trigger is false.

**Proof:** assume otherwise that `ub_trigger` is true.

The following facts are true:
 - `!present(ai) ∨ !present(bi) ∨ l`
 - `!present(ai) ∨ present(bi)`. Consequence: ai is the bottom of the hierarchy, if it is present all others are.

By resolution we obtain: `ub_trigger ⊧ !present(ai) ∨ l`, equivalent to `ub_trigger ⊧ present(ai) ⇒ l`

If `ai` is present then all other variables are, thus all edges become fully active in both direction: we have a negative cycle.
THis means that `Model ⊧ present(ai) ⇒ !l`

By resolution with the other implication, we obtain `!present(ai)`.



**Other dir** ub_trigger is false.
  - (present(ai) ∧ present(bi) ∧ !l) ∨ (present(ai) ∧ !present(bi))
  - `present(ai) \wedge [ (p(bi) ∧ !l) ∨ !p(bi) `


On veut montrer que si p(ai) & p(bi) alors !l

 - Si les deux sont présent, alors tout les autres arc sont actif.
 - de plus p(ai) <=> p(bi)
 - Si l = true, alors l'arc inactif deviendrait actif ce qui donnerait un cycle negatif
 - donc `pai & pbi => !l`
 - !pai | !pbi | !l

dans le cas du support on a l => pai & pbi, cad
 - !pai => !l
 - !pbi => !l

Dans ce cas là on peut en déduire `!l`

**Question ouverte**: est-ce que notre sémantique impose `l => pai & pbi` ?
Ça a du sens, dans le cas contraire, ai et bi n'ont pas de valeur entière qui puisse être comparée.