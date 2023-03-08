


A disjunction a | b | c is defined when
 - pa & a
 - pb & b
 - pc & c
 - pa & pb & pc    (this case is only non-overlaping with teh otehr when !a & !b & !c)


A conjunction a & b & c is defined when
- pa & !a
- pb & !b
- pc & !c
- pa & pb & pc    (this case is only non-overlaping with teh otehr when a & b & c)



supported_by_x   =   py & (x=y)     [px]
- defined when [px & py]
- defined when !py  ||  (px & py)

py & x=y => supported_by_x
!py || x!=y || supported_by_x
py & !supported_by_x => x!=y
!supported_by_x & x=y => !py