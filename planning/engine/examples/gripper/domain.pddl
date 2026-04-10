(define (domain gripper_x_1-domain)
 (:requirements :strips :typing)
 (:types room ball gripper)
 (:predicates (at_robby ?r - room) (at_ ?b - ball ?r - room) (free ?g - gripper) (carry ?o - ball ?g - gripper))
 (:action move
  :parameters ( ?from - room ?to - room)
  :precondition (and (at_robby ?from))
  :effect (and (at_robby ?to) (not (at_robby ?from))))
 (:action pick
  :parameters ( ?obj - ball ?room - room ?gripper - gripper)
  :precondition (and (at_ ?obj ?room) (at_robby ?room) (free ?gripper))
  :effect (and (carry ?obj ?gripper) (not (at_ ?obj ?room)) (not (free ?gripper))))
 (:action drop
  :parameters ( ?obj - ball ?room - room ?gripper - gripper)
  :precondition (and (carry ?obj ?gripper) (at_robby ?room))
  :effect (and (at_ ?obj ?room) (free ?gripper) (not (carry ?obj ?gripper))))
)
