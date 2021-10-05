# Extension for optionals

Consider the case where be have to optional boolean variables:

 - `a` that is in scope α
 - `b` that is in scope β

 The boolean disjunction \\( a \vee b \\) is:

 - `true` if `a` is present and true or if `b` is present and true   `(a ∧ α) ∨ (b ∧ β)`
 - `false` if `a` is present and false and if `b` is  present and false `(¬a ∧ α) ∨ (¬b ∧ β)`
 - `⊥` (undefined) if `(¬a ∧ ¬β) ∨ (¬b ∧ ¬α) ∨ (¬α ∧ ¬β)`


\\[ (a_\alpha \vee b_\beta)_{\alpha\beta} \\]



# Sufficient conditions for propagation

## Simple case

If both `a` and `b` are present (i.e. we are in the conjunctive scope αβ), then all components of the clause are present and be can 



!((a && x) || (b && y)) || !( !a && x && !b && y) 



## WIP

We have a disjunction `¬a ∨ ¬b ∨ c`, a model `M`.

We say that `M ⊨ a` if `a ∈ { true, ⊥}`.

- If `M ⊨ a, b`, can be propagate `c ≠ false` ?

  - if `α ∧ β`, meaning that `a = true`  and `b = true`    (base condition in non-optional reasoning)
  - if `¬γ`, meaning that `c = ⊥`   (condition where propagation is correct but useless)



If we know that `(α ∧ β) ∨ ¬γ` holds, we can always propagate because we known that we will always be in one of the two valid propagation conditions.
Let's reformulate it:

 - `γ ⇒ (α ∧ β)`


 ### With validity scope

 Lets call `π` the conjunctive scope of all conjuncts: `π = (α ∧ β ∧ γ)`.

 **Claim:** When unit propagation infers that a literal `a` of the clause is true, it makes this inference under the assumption that the the clause is within its validity scope.
 In other words, it infers that `π ⇒ a`.

 By construction, the literal `π` is always present.
 When I learn `π ⇒ a`, when can i deduce that `l ≠ false` (i.e. `l ∈ { ⊥, true}`).

 - when `α ⇒ π`, proof:
   - assume `¬α` then asserting `l ≠ false` has no effect.
   - assume `α` then the premise `π` will hold.




 ## Example

 We have an action A with presence `α` and parameter `xa`. It can be supported 
 
  - by action B with presence `β` and parameter `xb`
    - \\( subB_α  ⇔  [β ∧ (xa_α = xb_β)_{αβ}]_α   \\)
  - by action C with presence `γ` and parameter `xc`


Constraints:

- \\( ¬α ∨ subB_α ∨ subC_α \\)
  - `¬subB ∧ ¬subC ⇒ ¬α` ✓ (but not the same conditions)
  - `α ∧ ¬subB ⇒ subC` ✓
  - `α ∧ ¬subC ⇒ subB` ✓
- \\( subB_α  ⇔  [β ∧ (xa_α = xb_β)_{αβ}]_α   \\)
  - `¬subB ∨ β`
    - `subB ⇒ β` x    |> should be `subB ⇒ (α ⇒ β)`
    - `¬β ⇒ ¬subB` ✓
  - `¬subB ∨ (xa=xb)`
    - `subB ⇒ (xa=xb)` ✓
    - `(xa≠xb) → ¬subB` ✓
  - `(xa≠xb) ∨ ¬β ∨ subB`
    - `(xa=xb) ∧ β ⇒ subB` ✓
    - `(xa=xb) ∧ ¬subB ⇒ ¬β` x   |> should be `... ⇒ (α ⇒ ¬β)`
    - `β ∧ ¬subB ⇒ (xa≠xb)` ✓
- subC ...

Propagation desiderata:

 - if we learn \\( (xa_α = xb_β) ≠ true \\)
   - infer `subB ≠ true`
   - infer `subC ≠ false`

- if we learn  