(define (problem p-problem)
 (:domain p-domain)
 (:objects
   p0 - passenger
   f0 f1 - floor
 )
 (:htn
  :ordered-subtasks (and
    (task0 (achieve_served p0))))
 (:init (above f0 f1) (origin p0 f1) (destin p0 f0) (lift_at f0))
 (:goal (and ))
)
