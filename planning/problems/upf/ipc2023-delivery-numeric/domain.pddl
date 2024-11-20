(define (domain delivery_x_1-domain)
 (:requirements :strips :typing :numeric-fluents)
 (:types room item arm bot)
 (:predicates (at_bot ?b - bot ?x - room) (at_ ?i - item ?x - room) (door ?x - room ?y - room) (free ?a - arm) (in_arm ?i - item ?a - arm) (in_tray ?i - item ?b - bot) (mount ?a - arm ?b - bot))
 (:functions (load_limit ?b - bot) (current_load ?b - bot) (weight ?i - item) (cost))
 (:action move
  :parameters ( ?b - bot ?x - room ?y - room)
  :precondition (and (at_bot ?b ?x) (door ?x ?y))
  :effect (and (at_bot ?b ?y) (not (at_bot ?b ?x)) (increase (cost) 3)))
 (:action pick
  :parameters ( ?i - item ?x - room ?a - arm ?b - bot)
  :precondition (and (at_ ?i ?x) (at_bot ?b ?x) (free ?a) (mount ?a ?b) (<= (+ (weight ?i) (current_load ?b)) (load_limit ?b)))
  :effect (and (in_arm ?i ?a) (not (at_ ?i ?x)) (not (free ?a)) (increase (current_load ?b) (weight ?i)) (increase (cost) 2)))
 (:action drop
  :parameters ( ?i - item ?x - room ?a - arm ?b - bot)
  :precondition (and (in_arm ?i ?a) (at_bot ?b ?x) (mount ?a ?b))
  :effect (and (free ?a) (at_ ?i ?x) (not (in_arm ?i ?a)) (decrease (current_load ?b) (weight ?i)) (increase (cost) 2)))
 (:action to_tray
  :parameters ( ?i - item ?a - arm ?b - bot)
  :precondition (and (in_arm ?i ?a) (mount ?a ?b))
  :effect (and (free ?a) (not (in_arm ?i ?a)) (in_tray ?i ?b) (increase (cost) 1)))
 (:action from_tray
  :parameters ( ?i - item ?a - arm ?b - bot)
  :precondition (and (in_tray ?i ?b) (mount ?a ?b) (free ?a))
  :effect (and (not (free ?a)) (in_arm ?i ?a) (not (in_tray ?i ?b)) (increase (cost) 1)))
)
