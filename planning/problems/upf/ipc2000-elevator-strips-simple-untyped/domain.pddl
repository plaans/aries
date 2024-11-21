(define (domain mixed_f2_p1_u0_v0_g0_a0_n0_a0_b0_n0_f0_r0-domain)
 (:requirements :strips :typing)
 (:predicates (origin ?person - object ?floor - object) (floor ?floor - object) (passenger ?passenger - object) (destin ?person - object ?floor - object) (above ?floor1 - object ?floor2 - object) (boarded ?person - object) (served ?person - object) (lift_at ?floor - object))
 (:action board
  :parameters ( ?f - object ?p - object)
  :precondition (and (floor ?f) (passenger ?p) (lift_at ?f) (origin ?p ?f))
  :effect (and (boarded ?p)))
 (:action depart
  :parameters ( ?f - object ?p - object)
  :precondition (and (floor ?f) (passenger ?p) (lift_at ?f) (destin ?p ?f) (boarded ?p))
  :effect (and (not (boarded ?p)) (served ?p)))
 (:action up
  :parameters ( ?f1 - object ?f2 - object)
  :precondition (and (floor ?f1) (floor ?f2) (lift_at ?f1) (above ?f1 ?f2))
  :effect (and (lift_at ?f2) (not (lift_at ?f1))))
 (:action down
  :parameters ( ?f1 - object ?f2 - object)
  :precondition (and (floor ?f1) (floor ?f2) (lift_at ?f1) (above ?f2 ?f1))
  :effect (and (lift_at ?f2) (not (lift_at ?f1))))
)
