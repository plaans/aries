(define (domain instance_4-domain)
 (:requirements :strips :typing :numeric-fluents)
 (:types counter)
 (:predicates  (dummy-predicate))
 (:functions (value ?c - counter) (max_int))
 (:action increment
  :parameters ( ?c - counter)
  :precondition (and (<= (+ 1 (value ?c)) (max_int)))
  :effect (and (increase (value ?c) 1)))
 (:action decrement
  :parameters ( ?c - counter)
  :precondition (and (<= 1 (value ?c)))
  :effect (and (decrease (value ?c) 1)))
)
