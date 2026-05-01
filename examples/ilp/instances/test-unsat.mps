NAME          test-unsat
ROWS
 N  obj
 L  c1
 L  c2
 L  c3
 L  c4
 L  c5
 G  c6
COLUMNS
    MARK0000  'MARKER'  'INTORG'
    x1        obj       0         c1        1
    x1        c5        1         c6        1
    x2        c1        1         c2        1
    x2        c6        1
    x3        c2        1         c3        1
    x3        c6        1
    x4        c3        1         c4        1
    x4        c6        1
    x5        c4        1         c5        1
    x5        c6        1
    MARK0001  'MARKER'  'INTEND'
RHS
    rhs       c1        1         c2        1
    rhs       c3        1         c4        1
    rhs       c5        1         c6        3
BOUNDS
 UI bnd       x1        1
 UI bnd       x2        1
 UI bnd       x3        1
 UI bnd       x4        1
 UI bnd       x5        1
ENDATA