(define (domain tower_problem_1-domain)
 (:requirements :strips :typing :hierarchy :method-preconditions)
 (:types
    obj - object
    ring tower - obj
 )
 (:predicates (on ?r - ring ?o - obj) (goal_on ?r - ring ?o - obj) (towertop ?o - obj ?t - tower) (smallerthan ?r - ring ?o - obj))
 (:task shifttower
  :parameters ( ?t1 - tower ?t2 - tower ?t3 - tower))
 (:task selectdirection
  :parameters ( ?r - ring ?t1 - tower ?t2 - tower ?t3 - tower))
 (:task rotatetower
  :parameters ( ?t1 - tower ?t2 - tower ?t3 - tower))
 (:task exchange
  :parameters ( ?t1 - tower ?t2 - tower ?t3 - tower))
 (:task move_abstract
  :parameters ( ?t1 - tower ?t2 - tower))
 (:method m_shifttower
  :parameters ( ?r - ring ?t1 - tower ?t2 - tower ?t3 - tower)
  :task (shifttower ?t1 ?t2 ?t3)
  :precondition (and (towertop ?r ?t1))
  :ordered-subtasks (and
    (_t1842 (selectdirection ?r ?t1 ?t2 ?t3))))
 (:method selecteddirection
  :parameters ( ?r - ring ?t1 - tower ?t2 - tower ?t3 - tower)
  :task (selectdirection ?r ?t1 ?t2 ?t3)
  :precondition (and (on ?r ?t1))
  :ordered-subtasks (and
    (_t1843 (rotatetower ?t1 ?t3 ?t2))))
 (:method m_selectdirection
  :parameters ( ?r - ring ?r1 - ring ?t1 - tower ?t2 - tower ?t3 - tower)
  :task (selectdirection ?r ?t1 ?t2 ?t3)
  :precondition (and (on ?r ?r1))
  :ordered-subtasks (and
    (_t1844 (selectdirection ?r1 ?t1 ?t3 ?t2))))
 (:method m_rotatetower
  :parameters ( ?t1 - tower ?t2 - tower ?t3 - tower)
  :task (rotatetower ?t1 ?t2 ?t3)
  :ordered-subtasks (and
    (_t1845 (move_abstract ?t1 ?t2))
    (_t1846 (exchange ?t1 ?t2 ?t3))))
 (:method exchangeclear
  :parameters ( ?t1 - tower ?t2 - tower ?t3 - tower)
  :task (exchange ?t1 ?t2 ?t3)
  :precondition (and (towertop ?t1 ?t1) (towertop ?t3 ?t3)))
 (:method exchangelr
  :parameters ( ?r1 - ring ?o3 - obj ?t1 - tower ?t2 - tower ?t3 - tower)
  :task (exchange ?t1 ?t2 ?t3)
  :precondition (and (towertop ?r1 ?t1) (towertop ?o3 ?t3) (smallerthan ?r1 ?o3))
  :ordered-subtasks (and
    (_t1847 (move_abstract ?t1 ?t3))
    (_t1848 (rotatetower ?t2 ?t3 ?t1))))
 (:method exchangerl
  :parameters ( ?o1 - obj ?r3 - ring ?t1 - tower ?t2 - tower ?t3 - tower)
  :task (exchange ?t1 ?t2 ?t3)
  :precondition (and (towertop ?o1 ?t1) (towertop ?r3 ?t3) (smallerthan ?r3 ?o1))
  :ordered-subtasks (and
    (_t1849 (move_abstract ?t3 ?t1))
    (_t1850 (rotatetower ?t2 ?t3 ?t1))))
 (:method newmethod21
  :parameters ( ?r - ring ?o1 - obj ?t1 - tower ?o2 - obj ?t2 - tower)
  :task (move_abstract ?t1 ?t2)
  :ordered-subtasks (and
    (_t1851 (move ?r ?o1 ?t1 ?o2 ?t2))))
 (:action move
  :parameters ( ?r - ring ?o1 - obj ?t1 - tower ?o2 - obj ?t2 - tower)
  :precondition (and (towertop ?r ?t1) (towertop ?o2 ?t2) (on ?r ?o1) (smallerthan ?r ?o2))
  :effect (and (not (on ?r ?o1)) (on ?r ?o2) (not (towertop ?r ?t1)) (towertop ?o1 ?t1) (not (towertop ?o2 ?t2)) (towertop ?r ?t2)))
)
