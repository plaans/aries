% The nqueens puzzle is the problem of placing n chess queens 
% on an n × n chessboard so that no two queens threaten each other.
% Thus, a solution requires that no two queens share the same 
% row, column, or diagonal.

include "alldifferent.mzn";

int: n; % number of queens
array [1..n] of var 1..n: q; % queen in column i is in row q[i]

constraint assert(n >= 0, "n must be positive");

constraint alldifferent(q);                       % distinct rows
constraint alldifferent([ q[i] + i | i in 1..n]); % distinct upwards diagonals
constraint alldifferent([ q[i] - i | i in 1..n]); % distinct downwards diagonals

solve satisfy;

output [ if fix(q[j]) == i then "Q" else "." endif ++
         if j == n then "\n" else "" endif | i,j in 1..n];