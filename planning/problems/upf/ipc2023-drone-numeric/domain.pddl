(define (domain name-domain)
 (:requirements :strips :typing :equality :numeric-fluents)
 (:types location)
 (:predicates (visited ?x - location))
 (:functions (x) (y) (z) (xl ?l - location) (yl ?l - location) (zl ?l - location) (battery_level) (battery_level_full) (min_x) (max_x) (min_y) (max_y) (min_z) (max_z))
 (:action increase_x
  :parameters ()
  :precondition (and (<= 1 (battery_level)) (<= (x) (- (max_x) 1)))
  :effect (and (increase (x) 1) (decrease (battery_level) 1)))
 (:action decrease_x
  :parameters ()
  :precondition (and (<= 1 (battery_level)) (<= (+ 1 (min_x)) (x)))
  :effect (and (decrease (x) 1) (decrease (battery_level) 1)))
 (:action increase_y
  :parameters ()
  :precondition (and (<= 1 (battery_level)) (<= (y) (- (max_y) 1)))
  :effect (and (increase (y) 1) (decrease (battery_level) 1)))
 (:action decrease_y
  :parameters ()
  :precondition (and (<= 1 (battery_level)) (<= (+ 1 (min_y)) (y)))
  :effect (and (decrease (y) 1) (decrease (battery_level) 1)))
 (:action increase_z
  :parameters ()
  :precondition (and (<= 1 (battery_level)) (<= (z) (- (max_z) 1)))
  :effect (and (increase (z) 1) (decrease (battery_level) 1)))
 (:action decrease_z
  :parameters ()
  :precondition (and (<= 1 (battery_level)) (<= (+ 1 (min_z)) (z)))
  :effect (and (decrease (z) 1) (decrease (battery_level) 1)))
 (:action visit
  :parameters ( ?l - location)
  :precondition (and (<= 1 (battery_level)) (= (xl ?l) (x)) (= (yl ?l) (y)) (= (zl ?l) (z)))
  :effect (and (visited ?l) (decrease (battery_level) 1)))
 (:action recharge
  :parameters ()
  :precondition (and (= (x) 0) (= (y) 0) (= (z) 0))
  :effect (and (assign (battery_level) (battery_level_full))))
)
