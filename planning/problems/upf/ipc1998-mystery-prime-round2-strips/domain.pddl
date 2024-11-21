(define (domain strips_mprime_y_1-domain)
 (:requirements :strips :typing :negative-preconditions :equality)
 (:predicates (province ?x - object) (planet ?x - object) (food ?x - object) (pleasure ?x - object) (pain ?x - object) (eats ?n1 - object ?n2 - object) (craves ?v - object ?n - object) (fears ?c - object ?v - object) (locale ?n - object ?a - object) (harmony ?v - object ?s - object) (attacks ?i - object ?j - object) (orbits ?i - object ?j - object))
 (:action overcome
  :parameters ( ?c - object ?v - object ?n - object ?s1 - object ?s2 - object)
  :precondition (and (pain ?c) (pleasure ?v) (craves ?c ?n) (craves ?v ?n) (food ?n) (harmony ?v ?s2) (planet ?s2) (orbits ?s1 ?s2) (planet ?s1))
  :effect (and (not (craves ?c ?n)) (fears ?c ?v) (not (harmony ?v ?s2)) (harmony ?v ?s1)))
 (:action feast
  :parameters ( ?v - object ?n1 - object ?n2 - object ?l1 - object ?l2 - object)
  :precondition (and (craves ?v ?n1) (food ?n1) (pleasure ?v) (eats ?n1 ?n2) (food ?n2) (locale ?n1 ?l2) (attacks ?l1 ?l2))
  :effect (and (not (craves ?v ?n1)) (craves ?v ?n2) (not (locale ?n1 ?l2)) (locale ?n1 ?l1)))
 (:action succumb
  :parameters ( ?c - object ?v - object ?n - object ?s1 - object ?s2 - object)
  :precondition (and (fears ?c ?v) (pain ?c) (pleasure ?v) (craves ?v ?n) (food ?n) (harmony ?v ?s1) (orbits ?s1 ?s2))
  :effect (and (not (fears ?c ?v)) (craves ?c ?n) (not (harmony ?v ?s1)) (harmony ?v ?s2)))
 (:action drink
  :parameters ( ?n1 - object ?n2 - object ?l11 - object ?l12 - object ?l13 - object ?l21 - object ?l22 - object)
  :precondition (and (not (= ?n1 ?n2)) (locale ?n1 ?l11) (attacks ?l12 ?l11) (attacks ?l13 ?l12) (locale ?n2 ?l21) (attacks ?l21 ?l22))
  :effect (and (not (locale ?n1 ?l11)) (locale ?n1 ?l12) (not (locale ?n2 ?l21)) (locale ?n2 ?l22)))
)
