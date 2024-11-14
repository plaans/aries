(define (domain strips_gripper_x_1-domain)
 (:requirements :strips :typing)
 (:predicates (room ?r - object) (ball ?b - object) (gripper ?g - object) (at_robby ?r - object) (at_ ?b - object ?r - object) (free ?g - object) (carry ?o - object ?g - object))
 (:action move
  :parameters ( ?from - object ?to - object)
  :precondition (and (room ?from) (room ?to) (at_robby ?from))
  :effect (and (at_robby ?to) (not (at_robby ?from))))
 (:action pick
  :parameters ( ?obj - object ?room - object ?gripper - object)
  :precondition (and (ball ?obj) (room ?room) (gripper ?gripper) (at_ ?obj ?room) (at_robby ?room) (free ?gripper))
  :effect (and (carry ?obj ?gripper) (not (at_ ?obj ?room)) (not (free ?gripper))))
 (:action drop
  :parameters ( ?obj - object ?room - object ?gripper - object)
  :precondition (and (ball ?obj) (room ?room) (gripper ?gripper) (carry ?obj ?gripper) (at_robby ?room))
  :effect (and (at_ ?obj ?room) (free ?gripper) (not (carry ?obj ?gripper))))
)
