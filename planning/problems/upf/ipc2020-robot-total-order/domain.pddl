(define (domain pfile_01_001-domain)
 (:requirements :strips :typing :negative-preconditions :hierarchy :method-preconditions)
 (:types package room roomdoor)
 (:predicates (armempty) (rloc ?loc - room) (in ?obj - package ?loc - room) (holding ?obj - package) (closed ?d - roomdoor) (door ?loc1 - room ?loc2 - room ?d - roomdoor) (goal_in ?obj - package ?loc - room))
 (:task achieve_goals
  :parameters ())
 (:task release
  :parameters ())
 (:task pickup_abstract
  :parameters ( ?obj - package))
 (:task putdown_abstract
  :parameters ())
 (:task move_abstract
  :parameters ())
 (:task open_abstract
  :parameters ())
 (:method release_putdown_abstract
  :parameters ( ?loc - room ?obj - package)
  :task (release )
  :precondition (and (rloc ?loc) (holding ?obj) (goal_in ?obj ?loc))
  :ordered-subtasks (and
    (_t1712 (putdown_abstract ))
    (_t1713 (achieve_goals ))))
 (:method release_move
  :parameters ()
  :task (release )
  :ordered-subtasks (and
    (_t1714 (move_abstract ))
    (_t1715 (release ))))
 (:method release_open
  :parameters ()
  :task (release )
  :ordered-subtasks (and
    (_t1716 (open_abstract ))
    (_t1717 (release ))))
 (:method achieve_goals_pickup
  :parameters ( ?loc - room ?obj - package)
  :task (achieve_goals )
  :precondition (and (rloc ?loc) (in ?obj ?loc) (not (goal_in ?obj ?loc)))
  :ordered-subtasks (and
    (_t1718 (pickup_abstract ?obj))
    (_t1719 (release ))))
 (:method achieve_goals_move
  :parameters ()
  :task (achieve_goals )
  :ordered-subtasks (and
    (_t1720 (move_abstract ))
    (_t1721 (achieve_goals ))))
 (:method achieve_goals_open
  :parameters ()
  :task (achieve_goals )
  :ordered-subtasks (and
    (_t1722 (open_abstract ))
    (_t1723 (achieve_goals ))))
 (:method finished
  :parameters ()
  :task (achieve_goals ))
 (:method newmethod22
  :parameters ( ?obj - package ?loc - room)
  :task (pickup_abstract ?obj)
  :ordered-subtasks (and
    (_t1724 (pickup ?obj ?loc))))
 (:method newmethod23
  :parameters ( ?obj - package ?loc - room)
  :task (putdown_abstract )
  :ordered-subtasks (and
    (_t1725 (putdown ?obj ?loc))))
 (:method newmethod24
  :parameters ( ?loc1 - room ?loc2 - room ?d - roomdoor)
  :task (move_abstract )
  :ordered-subtasks (and
    (_t1726 (move ?loc1 ?loc2 ?d))))
 (:method newmethod25
  :parameters ( ?loc1 - room ?loc2 - room ?d - roomdoor)
  :task (open_abstract )
  :ordered-subtasks (and
    (_t1727 (open ?loc1 ?loc2 ?d))))
 (:action pickup
  :parameters ( ?obj - package ?loc - room)
  :precondition (and (armempty) (rloc ?loc) (in ?obj ?loc))
  :effect (and (not (in ?obj ?loc)) (not (armempty)) (holding ?obj)))
 (:action putdown
  :parameters ( ?obj - package ?loc - room)
  :precondition (and (rloc ?loc) (holding ?obj) (goal_in ?obj ?loc))
  :effect (and (not (holding ?obj)) (armempty) (in ?obj ?loc)))
 (:action move
  :parameters ( ?loc1 - room ?loc2 - room ?d - roomdoor)
  :precondition (and (rloc ?loc1) (door ?loc1 ?loc2 ?d) (not (closed ?d)))
  :effect (and (rloc ?loc2) (not (rloc ?loc1))))
 (:action open
  :parameters ( ?loc1 - room ?loc2 - room ?d - roomdoor)
  :precondition (and (rloc ?loc1) (door ?loc1 ?loc2 ?d) (closed ?d))
  :effect (and (not (closed ?d))))
)
