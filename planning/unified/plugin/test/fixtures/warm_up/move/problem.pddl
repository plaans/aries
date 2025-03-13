(define (problem simple-problem)
    (:domain simple-domain)
    (:objects
        r1 r2 - robot
        l1 l2 l3 - location)
    (:init (at r1 l1) (at r2 l1))
    (:goal (and (at r1 l3) (at r2 l3)))
    (:metric minimize (total-time))
)