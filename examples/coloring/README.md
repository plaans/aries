# Graph coloring

This crate reads a graph as a .col file and determines it's chromatic number.

The input file must contain newline seperated edge declarations in the following format:

e 0 1 (Declare an edge between nodes 0 and 1)

Lines starting with anything other than e will be ignored.

## Encoding

The model contains a variable representing the chromatic number,
a variable for each node representing it's color,
enforced neq constraints between each pair of adjacent edges,
and reified eq constraints between each pair of non-adjacent edges.

## Brancher

The default activity brancher is used with a simple heuristic which favours boolean variables.
This favors branching on node color equality, which causes the eq reasoner to work a lot more.
This is also very inneffective (\~200 000 decisions for FullIns3, compared to \~5 000 with no heuristic and \~100 with the opposite heuristic, branching on node color).

