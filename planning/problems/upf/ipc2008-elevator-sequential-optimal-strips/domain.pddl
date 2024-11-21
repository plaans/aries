(define (domain elevators_sequencedstrips_p8_3_1-domain)
 (:requirements :strips :typing :numeric-fluents)
 (:types
    elevator passenger count - object
    slow_elevator fast_elevator - elevator
 )
 (:predicates (passenger_at ?person - passenger ?floor - count) (boarded ?person - passenger ?lift - elevator) (lift_at ?lift - elevator ?floor - count) (reachable_floor ?lift - elevator ?floor - count) (above ?floor1 - count ?floor2 - count) (passengers ?lift - elevator ?n - count) (can_hold ?lift - elevator ?n - count) (next ?n1 - count ?n2 - count))
 (:functions (total_cost) (travel_slow ?f1 - count ?f2 - count) (travel_fast ?f1 - count ?f2 - count))
 (:action move_up_slow
  :parameters ( ?lift_0 - slow_elevator ?f1 - count ?f2 - count)
  :precondition (and (lift_at ?lift_0 ?f1) (above ?f1 ?f2) (reachable_floor ?lift_0 ?f2))
  :effect (and (lift_at ?lift_0 ?f2) (not (lift_at ?lift_0 ?f1)) (increase (total_cost) (travel_slow ?f1 ?f2))))
 (:action move_down_slow
  :parameters ( ?lift_0 - slow_elevator ?f1 - count ?f2 - count)
  :precondition (and (lift_at ?lift_0 ?f1) (above ?f2 ?f1) (reachable_floor ?lift_0 ?f2))
  :effect (and (lift_at ?lift_0 ?f2) (not (lift_at ?lift_0 ?f1)) (increase (total_cost) (travel_slow ?f2 ?f1))))
 (:action move_up_fast
  :parameters ( ?lift_1 - fast_elevator ?f1 - count ?f2 - count)
  :precondition (and (lift_at ?lift_1 ?f1) (above ?f1 ?f2) (reachable_floor ?lift_1 ?f2))
  :effect (and (lift_at ?lift_1 ?f2) (not (lift_at ?lift_1 ?f1)) (increase (total_cost) (travel_fast ?f1 ?f2))))
 (:action move_down_fast
  :parameters ( ?lift_1 - fast_elevator ?f1 - count ?f2 - count)
  :precondition (and (lift_at ?lift_1 ?f1) (above ?f2 ?f1) (reachable_floor ?lift_1 ?f2))
  :effect (and (lift_at ?lift_1 ?f2) (not (lift_at ?lift_1 ?f1)) (increase (total_cost) (travel_fast ?f2 ?f1))))
 (:action board
  :parameters ( ?p - passenger ?lift - elevator ?f - count ?n1 - count ?n2 - count)
  :precondition (and (lift_at ?lift ?f) (passenger_at ?p ?f) (passengers ?lift ?n1) (next ?n1 ?n2) (can_hold ?lift ?n2))
  :effect (and (not (passenger_at ?p ?f)) (boarded ?p ?lift) (not (passengers ?lift ?n1)) (passengers ?lift ?n2)))
 (:action leave
  :parameters ( ?p - passenger ?lift - elevator ?f - count ?n1 - count ?n2 - count)
  :precondition (and (lift_at ?lift ?f) (boarded ?p ?lift) (passengers ?lift ?n1) (next ?n2 ?n1))
  :effect (and (passenger_at ?p ?f) (not (boarded ?p ?lift)) (not (passengers ?lift ?n1)) (passengers ?lift ?n2)))
)
