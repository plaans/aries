(define (problem freecell2_4-problem)
 (:domain freecell2_4-domain)
 (:objects
   spadea diamonda club2 heart2 spade2 hearta diamond2 cluba diamond0 club0 heart0 spade0 - card
   diamond club heart spade - suitsort
   n0 n1 n2 n3 n4 - denomination
 )
 (:init (successor n1 n0) (successor n2 n1) (successor n3 n2) (successor n4 n3) (cellspace n2) (clear spadea) (on spadea spade2) (bottomcol spade2) (clear diamonda) (on diamonda hearta) (bottomcol hearta) (clear club2) (on club2 diamond2) (bottomcol diamond2) (clear heart2) (on heart2 cluba) (bottomcol cluba) (colspace n0) (value spadea n1) (suit spadea spade) (canstack spadea diamond2) (canstack spadea heart2) (value diamonda n1) (suit diamonda diamond) (canstack diamonda club2) (canstack diamonda spade2) (value club2 n2) (suit club2 club) (value heart2 n2) (suit heart2 heart) (value spade2 n2) (suit spade2 spade) (value hearta n1) (suit hearta heart) (canstack hearta club2) (canstack hearta spade2) (value diamond2 n2) (suit diamond2 diamond) (value cluba n1) (suit cluba club) (canstack cluba diamond2) (canstack cluba heart2) (home diamond0) (value diamond0 n0) (suit diamond0 diamond) (home club0) (value club0 n0) (suit club0 club) (home heart0) (value heart0 n0) (suit heart0 heart) (home spade0) (value spade0 n0) (suit spade0 spade))
 (:goal (and (home diamond2) (home club2) (home heart2) (home spade2)))
)
