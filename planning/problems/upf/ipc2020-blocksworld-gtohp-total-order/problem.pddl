(define (problem bw_rand_5-problem)
 (:domain bw_rand_5-domain)
 (:objects
   b1 b2 b3 b4 b5 - block
 )
 (:htn
  :ordered-subtasks (and
    (task1 (do_put_on b4 b2))
    (task2 (do_put_on b1 b4))
    (task3 (do_put_on b3 b1))))
 (:init (handempty) (ontable b1) (on b2 b3) (on b3 b5) (on b4 b1) (on b5 b4) (clear b2))
 (:goal (and (on b1 b4) (on b3 b1)))
)
