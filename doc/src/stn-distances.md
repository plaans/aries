# STN Distance Graph


## Graph representation

### Nodes

Each numeric variable `x` in the Difference Logic problem is split into two nodes:

 - `ub(x)`: the node representing the upper bound of the variable
 - `lb(x)`: the node representing the lower bound of the variable


A STN constraint `X ---- W -----> Y` is split in two edges that resepectively opperate on the upper and lower bounds of the varaibles.

- `ub(X) ----- W ------> ub(Y)`
- `lb(Y) ----- W ------> lb(X)`



## Propagation

Let `M` be a model: a function that associates each variable bound to a value.
The values are represented as a `BoundValue` object and enforce the property that for any two bound values `V1` and `V2`:

```
if V1 <= V2 then
   M(X) <= V1  entails M(X) <= V2
```
Essentially, it means that reducing a `BoundValue` is always a tightening of the domain the referenced variable, regardless of whether it applies to an upper or to a lower bound.


```
for each (source, target, weight) in edges:
  candidate = M(source) + weight
  if candidate < M(target)
     M(target) := candidate
```

## Propagators

For each reified constraints of the form `p == (A < B)`[^note that the delay term is omitted here] there are four possible propagations depending on 
 - the value of `p`
 - the bound of `A`/`B` that is updated



| premice | inference | watched once `p` known |
|---------|-----------|---------|
|  `p` & `A ≥ n` | `B ≥ n + 1`  | A lower bound |
|  `p` & `B ≤ n` | `A ≤ n - 1`  | B upper bound |
|  `¬p` & `A ≤ n` | `B ≤ n`  | A upper bound |
|  `¬p` & `B ≥ n` | `A ≥ n `  | B lower bound |

In practice each ill yield a single propagator (directed edge in the propagation graph). Once `p` or `¬p` becomes known, two of them will be activated.



## Shortest path computations (reduced distance)

A fully propagated and consistent STN will have the property that for any edge `X ---- W ----> Y`, `M(Y) <= M(X) + W`.


Given a consistent model `M`, this property allows us to introduce a *reduced distance graph* where all edges have positive values.
An edge `X ---- W ----> Y` in the distance graph is simply replaced by an edge ``X ---- [W + M(X) - M(Y)] ----> Y` in the *reduced* distance graph.

Since all such edges are guaranteed to have a positive length we can use Dijkstra's algorithm to compute shortest paths.


Let `rd(A, B)` and `d(A,B)` respectively denote the reduced distance and the true distance we can convert between the two:

 - `rd(A, B) = d(A,B) + M(A) - M(B)`
 - `d(A, B) = rd(A,B) - M(A) + M(B)`
 