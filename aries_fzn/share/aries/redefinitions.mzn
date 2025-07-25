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

predicate array_bool_xor(array [int] of var bool: as) =
    abort("array_bool_xor is not supported");


% --------------------------------------------------
% Supported predicates
predicate array_bool_element(var int: b, array [int] of bool: as, var bool: c);
predicate array_int_element(var int: b, array [int] of int: as, var int: c);
predicate array_var_bool_element(var int: b, array [int] of var bool: as, var bool: c);
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
predicate int_times(var int: a, var int: b, var int: c);
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
    abort("array_float_element is not supported");

predicate array_var_float_element(var int: b, array [int] of var float: as, var float: c) =
    abort("array_var_float_element is not supported");

predicate float_abs(var float: a, var float: b) =
    abort("float_abs is not supported");

predicate float_acos(var float: a, var float: b) =
    abort("float_acos is not supported");

predicate float_acosh(var float: a, var float: b) =
    abort("float_acosh is not supported");

predicate float_asin(var float: a, var float: b) =
    abort("float_asin is not supported");

predicate float_asinh(var float: a, var float: b) =
    abort("float_asinh is not supported");

predicate float_atan(var float: a, var float: b) =
    abort("float_atan is not supported");

predicate float_atanh(var float: a, var float: b) =
    abort("float_atanh is not supported");

% Added in 2.7.1
% predicate float_ceil(var float: x, var int: y) =
%     abort("float_ceil is not supported");

predicate float_cos(var float: a, var float: b) =
    abort("float_cos is not supported");

predicate float_cosh(var float: a, var float: b) =
    abort("float_cosh is not supported");

predicate float_div(var float: a, var float: b, var float: c) =
    abort("float_div is not supported");

predicate float_eq(var float: a, var float: b) =
    abort("float_eq is not supported");

predicate float_eq_reif(var float: a, var float: b, var bool: r) =
    abort("float_eq_reif is not supported");

predicate float_exp(var float: a, var float: b) =
    abort("float_exp is not supported");

% Added in 2.7.1
% predicate float_floor(var float: x, var int: y) =
%     abort("float_floor is not supported");

predicate float_in_reif(var float: a, float: b, float: c, var bool: r) =
    abort("float_in_reif is not supported");

predicate float_le(var float: a, var float: b) =
    abort("float_le is not supported");

predicate float_le_reif(var float: a, var float: b, var bool: r) =
    abort("float_le_reif is not supported");

predicate float_lin_eq(array [int] of float: as, array [int] of var float: bs, float: c) =
    abort("float_lin_eq is not supported");

predicate float_lin_eq_reif(array [int] of float: as, array [int] of var float: bs, float: c, var bool: r) =
    abort("float_lin_eq_reif is not supported");

predicate float_lin_le(array [int] of float: as, array [int] of var float: bs, float: c) =
    abort("float_lin_le is not supported");

predicate float_lin_le_reif(array [int] of float: as, array [int] of var float: bs, float: c, var bool: r) =
    abort("float_lin_le_reif is not supported");

predicate float_lin_lt(array [int] of float: as, array [int] of var float: bs, float: c) =
    abort("float_lin_lt is not supported");

predicate float_lin_lt_reif(array [int] of float: as, array [int] of var float: bs, float: c, var bool: r) =
    abort("float_lin_lt_reif is not supported");

predicate float_lin_ne(array [int] of float: as, array [int] of var float: bs, float: c) =
    abort("float_lin_ne is not supported");

predicate float_lin_ne_reif(array [int] of float: as, array [int] of var float: bs, float: c, var bool: r) =
    abort("float_lin_ne_reif is not supported");

predicate float_ln(var float: a, var float: b) =
    abort("float_ln is not supported");

predicate float_log10(var float: a, var float: b) =
    abort("float_log10 is not supported");

predicate float_log2(var float: a, var float: b) =
    abort("float_log2 is not supported");

predicate float_lt(var float: a, var float: b) =
    abort("float_lt is not supported");

predicate float_lt_reif(var float: a, var float: b, var bool: r) =
    abort("float_lt_reif is not supported");

predicate float_max(var float: a, var float: b, var float: c) =
    abort("float_max is not supported");

predicate float_min(var float: a, var float: b, var float: c) =
    abort("float_min is not supported");

predicate float_ne(var float: a, var float: b) =
    abort("float_ne is not supported");

predicate float_ne_reif(var float: a, var float: b, var bool: r) =
    abort("float_ne_reif is not supported");

predicate float_plus(var float: a, var float: b, var float: c) =
    abort("float_plus is not supported");

predicate float_pow(var float: x, var float: y, var float: z) =
    abort("float_pow is not supported");

% Added in 2.7.1
% predicate float_round(var float: x, var int: y) =
%     abort("float_round is not supported");

predicate float_sin(var float: a, var float: b) =
    abort("float_sin is not supported");

predicate float_sinh(var float: a, var float: b) =
    abort("float_sinh is not supported");

predicate float_sqrt(var float: a, var float: b) =
    abort("float_sqrt is not supported");

predicate float_tan(var float: a, var float: b) =
    abort("float_tan is not supported");

predicate float_tanh(var float: a, var float: b) =
    abort("float_tanh is not supported");

predicate float_times(var float: a, var float: b, var float: c) =
    abort("float_times is not supported");

predicate int2float(var int: x, var float: y) =
    abort("int2float is not supported");
