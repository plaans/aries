(define (problem pfile_01_001-problem)
 (:domain pfile_01_001-domain)
 (:objects
   o1 - package
   c r1 - room
   d01 - roomdoor
 )
 (:htn
  :ordered-subtasks (and
    (task0 (achieve_goals ))))
 (:init (rloc c) (armempty) (door c r1 d01) (door r1 c d01) (closed d01) (in o1 r1) (goal_in o1 r1))
 (:goal (and (in o1 r1)))
)
