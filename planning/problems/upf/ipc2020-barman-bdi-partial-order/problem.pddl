(define (problem p_1_2_2-problem)
 (:domain p_1_2_2-domain)
 (:objects
   dispenser1 dispenser2 - dispenser
   level1 level2 level3 - level
   left right - hand
   shot1 shot2 - shot
   shaker1 - shaker
   ingredient1 ingredient2 - ingredient
   cocktail1 - cocktail
 )
 (:htn
  :ordered-subtasks (and
    (_t62 (achievecontainsshotcocktail shot2 cocktail1))))
 (:init (ontable shaker1) (ontable shot1) (ontable shot2) (clean shaker1) (clean shot1) (clean shot2) (empty shaker1) (empty shot1) (empty shot2) (dispenses dispenser1 ingredient1) (dispenses dispenser2 ingredient2) (handempty left) (handempty right) (shakeremptylevel shaker1 level1) (shakerlevel shaker1 level1) (next level1 level1) (next level2 level2) (cocktailpart1 cocktail1 ingredient2) (cocktailpart2 cocktail1 ingredient1))
 (:goal (and ))
)
