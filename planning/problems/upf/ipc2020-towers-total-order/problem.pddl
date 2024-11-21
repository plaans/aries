(define (problem tower_problem_1-problem)
 (:domain tower_problem_1-domain)
 (:objects
   r1 - ring
   t1 t2 t3 - tower
 )
 (:htn
  :ordered-subtasks (and
    (task0 (shifttower t1 t2 t3))))
 (:init (smallerthan r1 t1) (smallerthan r1 t2) (smallerthan r1 t3) (on r1 t1) (towertop r1 t1) (towertop t2 t2) (towertop t3 t3) (goal_on r1 t3))
 (:goal (and (on r1 t3)))
)
