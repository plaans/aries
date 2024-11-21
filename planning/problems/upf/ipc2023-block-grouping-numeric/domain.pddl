(define (domain instance_20_5_2_1-domain)
 (:requirements :strips :typing :negative-preconditions :disjunctive-preconditions :equality :numeric-fluents)
 (:types block)
 (:predicates  (dummy-predicate))
 (:functions (x ?b - block) (y ?b - block) (max_x) (min_x) (max_y) (min_y))
 (:action move_block_up
  :parameters ( ?b - block)
  :precondition (and (<= (+ 1 (y ?b)) (max_y)))
  :effect (and (increase (y ?b) 1)))
 (:action move_block_down
  :parameters ( ?b - block)
  :precondition (and (<= (min_y) (- (y ?b) 1)))
  :effect (and (decrease (y ?b) 1)))
 (:action move_block_right
  :parameters ( ?b - block)
  :precondition (and (<= (+ 1 (x ?b)) (max_x)))
  :effect (and (increase (x ?b) 1)))
 (:action move_block_left
  :parameters ( ?b - block)
  :precondition (and (<= (min_x) (- (x ?b) 1)))
  :effect (and (decrease (x ?b) 1)))
)
