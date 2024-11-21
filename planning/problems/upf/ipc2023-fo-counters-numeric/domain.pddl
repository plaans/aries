(define (domain instance_2-domain)
 (:requirements :strips :typing :numeric-fluents :action-costs)
 (:types counter)
 (:predicates  (dummy-predicate))
 (:functions (value ?c - counter) (rate_value ?c - counter) (max_int) (total-cost))
 (:action increment
  :parameters ( ?c - counter)
  :precondition (and (<= (+ (rate_value ?c) (value ?c)) (max_int)))
  :effect (and (increase (value ?c) (rate_value ?c)) (increase (total-cost) 1)))
 (:action decrement
  :parameters ( ?c - counter)
  :precondition (and (<= 0 (- (value ?c) (rate_value ?c))))
  :effect (and (decrease (value ?c) (rate_value ?c)) (increase (total-cost) 1)))
 (:action increase_rate
  :parameters ( ?c - counter)
  :precondition (and (<= (+ 1 (rate_value ?c)) 10))
  :effect (and (increase (rate_value ?c) 1) (increase (total-cost) 1)))
 (:action decrement_rate
  :parameters ( ?c - counter)
  :precondition (and (<= 1 (rate_value ?c)))
  :effect (and (decrease (rate_value ?c) 1) (increase (total-cost) 1)))
)
