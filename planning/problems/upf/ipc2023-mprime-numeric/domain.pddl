(define (domain mprime_x_25-domain)
 (:requirements :strips :typing :numeric-fluents)
 (:types
    food emotion - object
    pleasure pain - emotion
 )
 (:predicates (eats ?n1 - food ?n2 - food) (craves ?v - emotion ?n - food) (fears ?c - pain ?v_0 - pleasure))
 (:functions (harmony ?v - emotion) (locale ?n - food))
 (:action overcome
  :parameters ( ?c - pain ?v_0 - pleasure ?n - food)
  :precondition (and (craves ?c ?n) (craves ?v_0 ?n) (<= 1 (harmony ?v_0)))
  :effect (and (not (craves ?c ?n)) (fears ?c ?v_0) (decrease (harmony ?v_0) 1)))
 (:action feast
  :parameters ( ?v_0 - pleasure ?n1 - food ?n2 - food)
  :precondition (and (craves ?v_0 ?n1) (eats ?n1 ?n2) (<= 1 (locale ?n1)))
  :effect (and (not (craves ?v_0 ?n1)) (craves ?v_0 ?n2) (decrease (locale ?n1) 1)))
 (:action succumb
  :parameters ( ?c - pain ?v_0 - pleasure ?n - food)
  :precondition (and (fears ?c ?v_0) (craves ?v_0 ?n))
  :effect (and (not (fears ?c ?v_0)) (craves ?c ?n) (increase (harmony ?v_0) 1)))
 (:action drink
  :parameters ( ?n1 - food ?n2 - food)
  :precondition (and (<= 1 (locale ?n1)))
  :effect (and (decrease (locale ?n1) 1) (increase (locale ?n2) 1)))
)
