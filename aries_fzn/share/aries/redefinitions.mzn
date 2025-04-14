% predicate array_int_element(var int: b, array [int] of int: as, var int: c) =
%     abort("array_int_element is not supported");

% This predicate is in redefinitions 2.0
% predicate array_int_maximum(var int: m, array [int] of var int: x) =
%     abort("array_int_maximum is not supported");

% This predicate is in redefinitions 2.0
% predicate array_int_minimum(var int: m, array [int] of var int: x) =
%     abort("array_int_minimum is not supported");

% predicate array_var_int_element(var int: b, array [int] of var int: as, var int: c) =
%     abort("array_var_int_element is not supported");

% predicate int_abs(var int: a, var int: b) =
%     abort("int_abs is not supported");

predicate int_div(var int: a, var int: b, var int: c) =
    abort("int_div is not supported");

% predicate int_eq(var int: a, var int: b) =
%     abort("int_eq is not supported");

% predicate int_eq_reif(var int: a, var int: b, var bool: r) =
%     abort("int_eq_reif is not supported");

% predicate int_le(var int: a, var int: b) =
%     abort("int_le is not supported");

% predicate int_le_reif(var int: a, var int: b, var bool: r) =
%     abort("int_le_reif is not supported");

% predicate int_lin_eq(array [int] of int: as, array [int] of var int: bs, int: c) =
%     abort("int_lin_eq is not supported");

% int_lin_eq_imp is supported
predicate int_lin_eq_imp(array [int] of int: as, array [int] of var int: bs, int: c, var bool: r);

predicate int_lin_eq_reif(array [int] of int: as, array [int] of var int: bs, int: c, var bool: r) =
    abort("int_lin_eq_reif is not supported");

% predicate int_lin_le(array [int] of int: as, array [int] of var int: bs, int: c) =
%     abort("int_lin_le is not supported");

% int_lin_le_imp is supported
predicate int_lin_le_imp(array [int] of int: as, array [int] of var int: bs, int: c, var bool: r);

% predicate int_lin_le_reif(array [int] of int: as, array [int] of var int: bs, int: c, var bool: r) =
%     abort("int_lin_le_reif is not supported");

% predicate int_lin_ne(array [int] of int: as, array [int] of var int: bs, int: c) =
%     abort("int_lin_ne is not supported");

predicate int_lin_ne_reif(array [int] of int: as, array [int] of var int: bs, int: c, var bool: r) =
    abort("int_lin_ne_reif is not supported");

predicate int_lt(var int: a, var int: b) =
    abort("int_lt is not supported");

predicate int_lt_reif(var int: a, var int: b, var bool: r) =
    abort("int_lt_reif is not supported");

predicate int_max(var int: a, var int: b, var int: c) =
    array_int_maximum(c, [a,b]);

predicate int_min(var int: a, var int: b, var int: c) =
    array_int_minimum(c, [a,b]);

predicate int_mod(var int: a, var int: b, var int: c) =
    abort("int_mod is not supported");

% predicate int_ne(var int: a, var int: b) =
%     abort("int_ne is not supported");

% predicate int_ne_reif(var int: a, var int: b, var bool: r) =
%     abort("int_ne_reif is not supported");

predicate int_plus(var int: a, var int: b, var int: c) =
    abort("int_plus is not supported");

predicate int_pow(var int: x, var int: y, var int: z) =
    abort("int_pow is not supported");

predicate int_times(var int: a, var int: b, var int: c) =
    abort("int_times is not supported");

predicate set_in(var int: x, set of int: S) =
    abort("set_in is not supported");

% predicate array_bool_and(array [int] of var bool: as, var bool: r) =
%     abort("array_bool_and is not supported");

predicate array_bool_element(var int: b, array [int] of bool: as, var bool: c) =
    abort("array_bool_element is not supported");

predicate array_bool_xor(array [int] of var bool: as) =
    abort("array_bool_xor is not supported");

predicate array_var_bool_element(var int: b, array [int] of var bool: as, var bool: c) =
    abort("array_var_bool_element is not supported");

% predicate bool2int(var bool: a, var int: b) =
%     abort("bool2int is not supported");

predicate bool_and(var bool: a, var bool: b, var bool: r) =
    array_bool_and([a,b], r);

% predicate bool_clause(array [int] of var bool: as, array [int] of var bool: bs) =
%     abort("bool_clause is not supported");

% predicate bool_eq(var bool: a, var bool: b) =
%     abort("bool_eq is not supported");

predicate bool_eq_reif(var bool: a, var bool: b, var bool: r) =
    abort("bool_eq_reif is not supported");

predicate bool_le(var bool: a, var bool: b) =
    abort("bool_le is not supported");

predicate bool_le_reif(var bool: a, var bool: b, var bool: r) =
    abort("bool_le_reif is not supported");

predicate bool_lin_eq(array [int] of int: as, array [int] of var bool: bs, var int: c) =
    abort("bool_lin_eq is not supported");

predicate bool_lin_le(array [int] of int: as, array [int] of var bool: bs, int: c) =
    abort("bool_lin_le is not supported");

predicate bool_lt(var bool: a, var bool: b) =
    abort("bool_lt is not supported");

predicate bool_lt_reif(var bool: a, var bool: b, var bool: r) =
    abort("bool_lt_reif is not supported");

predicate bool_not(var bool: a, var bool: b) =
    abort("bool_not is not supported");

predicate bool_or(var bool: a, var bool: b, var bool: r) =
    abort("bool_or is not supported");

predicate bool_xor(var bool: a, var bool: b, var bool: r) =
    abort("bool_xor is not supported");

predicate bool_xor(var bool: a, var bool: b) =
    abort("bool_xor is not supported");

predicate array_set_element(var int: b, array [int] of set of int: as, var set of int: c) =
    abort("array_set_element is not supported");

predicate array_var_set_element(var int: b, array [int] of var set of int: as, var set of int: c) =
    abort("array_var_set_element is not supported");

predicate set_card(var set of int: S, var int: x) =
    abort("set_card is not supported");

predicate set_diff(var set of int: x, var set of int: y, var set of int: r) =
    abort("set_diff is not supported");

predicate set_eq(var set of int: x, var set of int: y) =
    abort("set_eq is not supported");

predicate set_eq_reif(var set of int: x, var set of int: y, var bool: r) =
    abort("set_eq_reif is not supported");

predicate set_in(var int: x, var set of int: S) =
    abort("set_in is not supported");

predicate set_in_reif(var int: x, set of int: S, var bool: r) =
    abort("set_in_reif is not supported");

predicate set_in_reif(var int: x, var set of int: S, var bool: r) =
    abort("set_in_reif is not supported");

predicate set_intersect(var set of int: x, var set of int: y, var set of int: r) =
    abort("set_intersect is not supported");

predicate set_le(var set of int: x, var set of int: y) =
    abort("set_le is not supported");

predicate set_le_reif(var set of int: x, var set of int: y, var bool: r) =
    abort("set_le_reif is not supported");

predicate set_lt(var set of int: x, var set of int: y) =
    abort("set_lt is not supported");

predicate set_lt_reif(var set of int: x, var set of int: y, var bool: r) =
    abort("set_lt_reif is not supported");

predicate set_ne(var set of int: x, var set of int: y) =
    abort("set_ne is not supported");

predicate set_ne_reif(var set of int: x, var set of int: y, var bool: r) =
    abort("set_ne_reif is not supported");

predicate set_subset(var set of int: x, var set of int: y) =
    abort("set_subset is not supported");

predicate set_subset_reif(var set of int: x, var set of int: y, var bool: r) =
    abort("set_subset_reif is not supported");

predicate set_superset(var set of int: x, var set of int: y) =
    abort("set_superset is not supported");

predicate set_superset_reif(var set of int: x, var set of int: y, var bool: r) =
    abort("set_superset_reif is not supported");

predicate set_symdiff(var set of int: x, var set of int: y, var set of int: r) =
    abort("set_symdiff is not supported");

predicate set_union(var set of int: x, var set of int: y, var set of int: r) =
    abort("set_union is not supported");

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

