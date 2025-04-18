% Eliminate set variables by converting them into arrays of var bool
include "nosets.mzn";

% --------------------------------------------------
% Supported half reified constraints

predicate int_lin_eq_imp(array [int] of int: as, array [int] of var int: bs, int: c, var bool: r);
predicate int_lin_le_imp(array [int] of int: as, array [int] of var int: bs, int: c, var bool: r);


% --------------------------------------------------
% Redefined predicates

predicate int_max(var int: a, var int: b, var int: c) =
    array_int_maximum(c, [a,b]);

predicate int_min(var int: a, var int: b, var int: c) =
    array_int_minimum(c, [a,b]);

predicate int_plus(var int: a, var int: b, var int: c) =
    int_lin_eq([1, 1, -1], [a, b, c], 0);

predicate bool_and(var bool: a, var bool: b, var bool: r) =
    array_bool_and([a,b], r);

predicate bool_lt(var bool: a, var bool: b) =
    a = false /\ b = true;

predicate bool_lt_reif(var bool: a, var bool: b, var bool: r) =
    r <-> ((a = false) /\ (b = true));

predicate bool_or(var bool: a, var bool: b, var bool: r) =
    bool_clause_reif([a,b], [], r);


% --------------------------------------------------
% Predicates not supported

predicate int_div(var int: a, var int: b, var int: c) =
    abort("int_div is not supported");

predicate int_mod(var int: a, var int: b, var int: c) =
    abort("int_mod is not supported");

predicate int_pow(var int: x, var int: y, var int: z) =
    abort("int_pow is not supported");

predicate int_times(var int: a, var int: b, var int: c) =
    abort("int_times is not supported");

predicate array_bool_element(var int: b, array [int] of bool: as, var bool: c) =
    abort("array_bool_element is not supported");

predicate array_bool_xor(array [int] of var bool: as) =
    abort("array_bool_xor is not supported");

predicate array_var_bool_element(var int: b, array [int] of var bool: as, var bool: c) =
    abort("array_var_bool_element is not supported");


% --------------------------------------------------
% Supported predicates
predicate array_int_element(var int: b, array [int] of int: as, var int: c);
predicate array_var_int_element(var int: b, array [int] of var int: as, var int: c);
predicate int_abs(var int: a, var int: b);
predicate int_eq(var int: a, var int: b);
predicate int_eq_reif(var int: a, var int: b, var bool: r);
predicate int_le(var int: a, var int: b);
predicate int_le_reif(var int: a, var int: b, var bool: r);
predicate int_lin_eq(array [int] of int: as, array [int] of var int: bs, int: c);
predicate int_lin_eq_reif(array [int] of int: as, array [int] of var int: bs, int: c, var bool: r);
predicate int_lin_le(array [int] of int: as, array [int] of var int: bs, int: c);
predicate int_lin_le_reif(array [int] of int: as, array [int] of var int: bs, int: c, var bool: r);
predicate int_lin_ne(array [int] of int: as, array [int] of var int: bs, int: c);
predicate int_lin_ne_reif(array [int] of int: as, array [int] of var int: bs, int: c, var bool: r);
predicate int_lt(var int: a, var int: b);
predicate int_lt_reif(var int: a, var int: b, var bool: r);
predicate int_ne(var int: a, var int: b);
predicate int_ne_reif(var int: a, var int: b, var bool: r);
predicate set_in(var int: x, set of int: S);
predicate array_bool_and(array [int] of var bool: as, var bool: r);
predicate bool2int(var bool: a, var int: b);
predicate bool_clause(array [int] of var bool: as, array [int] of var bool: bs);
predicate bool_eq(var bool: a, var bool: b);
predicate bool_eq_reif(var bool: a, var bool: b, var bool: r);
predicate bool_le(var bool: a, var bool: b);
predicate bool_le_reif(var bool: a, var bool: b, var bool: r);
predicate bool_lin_eq(array [int] of int: as, array [int] of var bool: bs, var int: c);
predicate bool_lin_le(array [int] of int: as, array [int] of var bool: bs, int: c);
predicate bool_not(var bool: a, var bool: b);
predicate bool_xor(var bool: a, var bool: b, var bool: r);


% --------------------------------------------------
% The following predicates should be redefined by nosets.mzn
predicate array_set_element(var int: b, array [int] of set of int: as, var set of int: c);
predicate array_var_set_element(var int: b, array [int] of var set of int: as, var set of int: c);
predicate set_card(var set of int: S, var int: x);
predicate set_diff(var set of int: x, var set of int: y, var set of int: r);
predicate set_eq(var set of int: x, var set of int: y);
predicate set_eq_reif(var set of int: x, var set of int: y, var bool: r);
predicate set_in(var int: x, var set of int: S);
predicate set_in_reif(var int: x, var set of int: S, var bool: r);
predicate set_intersect(var set of int: x, var set of int: y, var set of int: r);
predicate set_le(var set of int: x, var set of int: y);
predicate set_le_reif(var set of int: x, var set of int: y, var bool: r);
predicate set_lt(var set of int: x, var set of int: y);
predicate set_lt_reif(var set of int: x, var set of int: y, var bool: r);
predicate set_ne(var set of int: x, var set of int: y);
predicate set_ne_reif(var set of int: x, var set of int: y, var bool: r);
predicate set_subset(var set of int: x, var set of int: y);
predicate set_subset_reif(var set of int: x, var set of int: y, var bool: r);
predicate set_superset(var set of int: x, var set of int: y);
predicate set_superset_reif(var set of int: x, var set of int: y, var bool: r);
predicate set_symdiff(var set of int: x, var set of int: y, var set of int: r);
predicate set_union(var set of int: x, var set of int: y, var set of int: r);


% --------------------------------------------------
% Floats are not supported

predicate array_float_element(var int: b, array [int] of float: as, var float: c) =
    abort("floats are not supported (array_float_element)");

predicate array_var_float_element(var int: b, array [int] of var float: as, var float: c) =
    abort("floats are not supported (array_var_float_element)");

predicate float_abs(var float: a, var float: b) =
    abort("floats are not supported (float_abs)");

predicate float_acos(var float: a, var float: b) =
    abort("floats are not supported (float_acos)");

predicate float_acosh(var float: a, var float: b) =
    abort("floats are not supported (float_acosh)");

predicate float_asin(var float: a, var float: b) =
    abort("floats are not supported (float_asin)");

predicate float_asinh(var float: a, var float: b) =
    abort("floats are not supported (float_asinh)");

predicate float_atan(var float: a, var float: b) =
    abort("floats are not supported (float_atan)");

predicate float_atanh(var float: a, var float: b) =
    abort("floats are not supported (float_atanh)");

% Added in 2.7.1
% predicate float_ceil(var float: x, var int: y) =
%     abort("floats are not supported (float_ceil)");

predicate float_cos(var float: a, var float: b) =
    abort("floats are not supported (float_cos)");

predicate float_cosh(var float: a, var float: b) =
    abort("floats are not supported (float_cosh)");

predicate float_div(var float: a, var float: b, var float: c) =
    abort("floats are not supported (float_div)");

predicate float_eq(var float: a, var float: b) =
    abort("floats are not supported (float_eq)");

predicate float_eq_reif(var float: a, var float: b, var bool: r) =
    abort("floats are not supported (float_eq_reif)");

predicate float_exp(var float: a, var float: b) =
    abort("floats are not supported (float_exp)");

% Added in 2.7.1
% predicate float_floor(var float: x, var int: y) =
%     abort("floats are not supported (float_floor)");

predicate float_in_reif(var float: a, float: b, float: c, var bool: r) =
    abort("floats are not supported (float_in_reif)");

predicate float_le(var float: a, var float: b) =
    abort("floats are not supported (float_le)");

predicate float_le_reif(var float: a, var float: b, var bool: r) =
    abort("floats are not supported (float_le_reif)");

predicate float_lin_eq(array [int] of float: as, array [int] of var float: bs, float: c) =
    abort("floats are not supported (float_lin_eq)");

predicate float_lin_eq_reif(array [int] of float: as, array [int] of var float: bs, float: c, var bool: r) =
    abort("floats are not supported (float_lin_eq_reif)");

predicate float_lin_le(array [int] of float: as, array [int] of var float: bs, float: c) =
    abort("floats are not supported (float_lin_le)");

predicate float_lin_le_reif(array [int] of float: as, array [int] of var float: bs, float: c, var bool: r) =
    abort("floats are not supported (float_lin_le_reif)");

predicate float_lin_lt(array [int] of float: as, array [int] of var float: bs, float: c) =
    abort("floats are not supported (float_lin_lt)");

predicate float_lin_lt_reif(array [int] of float: as, array [int] of var float: bs, float: c, var bool: r) =
    abort("floats are not supported (float_lin_lt_reif)");

predicate float_lin_ne(array [int] of float: as, array [int] of var float: bs, float: c) =
    abort("floats are not supported (float_lin_ne)");

predicate float_lin_ne_reif(array [int] of float: as, array [int] of var float: bs, float: c, var bool: r) =
    abort("floats are not supported (float_lin_ne_reif)");

predicate float_ln(var float: a, var float: b) =
    abort("floats are not supported (float_ln)");

predicate float_log10(var float: a, var float: b) =
    abort("floats are not supported (float_log10)");

predicate float_log2(var float: a, var float: b) =
    abort("floats are not supported (float_log2)");

predicate float_lt(var float: a, var float: b) =
    abort("floats are not supported (float_lt)");

predicate float_lt_reif(var float: a, var float: b, var bool: r) =
    abort("floats are not supported (float_lt_reif)");

predicate float_max(var float: a, var float: b, var float: c) =
    abort("floats are not supported (float_max)");

predicate float_min(var float: a, var float: b, var float: c) =
    abort("floats are not supported (float_min)");

predicate float_ne(var float: a, var float: b) =
    abort("floats are not supported (float_ne)");

predicate float_ne_reif(var float: a, var float: b, var bool: r) =
    abort("floats are not supported (float_ne_reif)");

predicate float_plus(var float: a, var float: b, var float: c) =
    abort("floats are not supported (float_plus)");

predicate float_pow(var float: x, var float: y, var float: z) =
    abort("floats are not supported (float_pow)");

% Added in 2.7.1
% predicate float_round(var float: x, var int: y) =
%     abort("floats are not supported (float_round)");

predicate float_sin(var float: a, var float: b) =
    abort("floats are not supported (float_sin)");

predicate float_sinh(var float: a, var float: b) =
    abort("floats are not supported (float_sinh)");

predicate float_sqrt(var float: a, var float: b) =
    abort("floats are not supported (float_sqrt)");

predicate float_tan(var float: a, var float: b) =
    abort("floats are not supported (float_tan)");

predicate float_tanh(var float: a, var float: b) =
    abort("floats are not supported (float_tanh)");

predicate float_times(var float: a, var float: b, var float: c) =
    abort("floats are not supported (float_times)");

predicate int2float(var int: x, var float: y) =
    abort("floats are not supported (int2float)");
