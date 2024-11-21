(define (domain mixed_f2_p1_u0_v0_g0_a0_n0_a0_b0_n0_f0_r0-domain)
 (:requirements :strips :typing)
 (:types passenger floor)
 (:predicates (origin ?person - passenger ?floor - floor) (destin ?person - passenger ?floor - floor) (above ?floor1 - floor ?floor2 - floor) (boarded ?person - passenger) (not_boarded ?person - passenger) (served ?person - passenger) (not_served ?person - passenger) (lift_at ?floor - floor))
 (:action board
  :parameters ( ?f - floor ?p - passenger)
  :precondition (and (lift_at ?f) (origin ?p ?f))
  :effect (and (boarded ?p)))
 (:action depart
  :parameters ( ?f - floor ?p - passenger)
  :precondition (and (lift_at ?f) (destin ?p ?f) (boarded ?p))
  :effect (and (not (boarded ?p)) (served ?p)))
 (:action up
  :parameters ( ?f1 - floor ?f2 - floor)
  :precondition (and (lift_at ?f1) (above ?f1 ?f2))
  :effect (and (lift_at ?f2) (not (lift_at ?f1))))
 (:action down
  :parameters ( ?f1 - floor ?f2 - floor)
  :precondition (and (lift_at ?f1) (above ?f2 ?f1))
  :effect (and (lift_at ?f2) (not (lift_at ?f1))))
)
