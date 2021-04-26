# STN Theory Propagation



## Edges

Upon the addition of a new edge AB, compute all shortest paths `O ------> T`that go through AB.

If there is an inactive edge `T -> O` such that the cycle `O -----> T -> O` has a negative length, the make all enablers of `T -> O` false.



## Bounds


On addition of a new upper bound `ub(X) = d`: it means we have a shortest path `0 ----> X` of length `d`


If there is an inactive edge `X -> Y` of length `w` such that:

 - `lb(Y) = d'`, meaning we have a shortest path `Y -----> 0` of length `-d'`
 - `d - d' + w < 0`

Then the `X -> Y` edge must be inactive as its addition would result in a negative cycle