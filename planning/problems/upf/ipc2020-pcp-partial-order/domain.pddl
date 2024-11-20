(define (domain someproblem-domain)
 (:requirements :strips :hierarchy)
 (:predicates (turna) (turnb) (lt1) (lt2) (lt3) (lx) (ly))
 (:task sg1
  :parameters ())
 (:task sg2
  :parameters ())
 (:method g1_s2t1_s_y
  :parameters ()
  :task (sg1 )
  :ordered-subtasks (and
    (task0 (t1g1 ))
    (task1 (sg1 ))
    (task2 (yg1 ))))
 (:method g1_s2t2_s_x_y
  :parameters ()
  :task (sg1 )
  :ordered-subtasks (and
    (task0 (t2g1 ))
    (task1 (sg1 ))
    (task2 (xg1 ))
    (task3 (yg1 ))))
 (:method g1_s2t3_s_y_y_x
  :parameters ()
  :task (sg1 )
  :ordered-subtasks (and
    (task0 (t3g1 ))
    (task1 (sg1 ))
    (task2 (yg1 ))
    (task3 (yg1 ))
    (task4 (xg1 ))))
 (:method g1_s2t1_y
  :parameters ()
  :task (sg1 )
  :ordered-subtasks (and
    (task0 (t1g1 ))
    (task1 (yg1 ))))
 (:method g1_s2t2_x_y
  :parameters ()
  :task (sg1 )
  :ordered-subtasks (and
    (task0 (t2g1 ))
    (task1 (xg1 ))
    (task2 (yg1 ))))
 (:method g1_s2t3_y_y_x
  :parameters ()
  :task (sg1 )
  :ordered-subtasks (and
    (task0 (t3g1 ))
    (task1 (yg1 ))
    (task2 (yg1 ))
    (task3 (xg1 ))))
 (:method g2_s2t1_s_y_x_y
  :parameters ()
  :task (sg2 )
  :ordered-subtasks (and
    (task0 (t1g2 ))
    (task1 (sg2 ))
    (task2 (yg2 ))
    (task3 (xg2 ))
    (task4 (yg2 ))))
 (:method g2_s2t2_s_x_x
  :parameters ()
  :task (sg2 )
  :ordered-subtasks (and
    (task0 (t2g2 ))
    (task1 (sg2 ))
    (task2 (xg2 ))
    (task3 (xg2 ))))
 (:method g2_s2t3_s_y_y
  :parameters ()
  :task (sg2 )
  :ordered-subtasks (and
    (task0 (t3g2 ))
    (task1 (sg2 ))
    (task2 (yg2 ))
    (task3 (yg2 ))))
 (:method g2_s2t1_y_x_y
  :parameters ()
  :task (sg2 )
  :ordered-subtasks (and
    (task0 (t1g2 ))
    (task1 (yg2 ))
    (task2 (xg2 ))
    (task3 (yg2 ))))
 (:method g2_s2t2_x_x
  :parameters ()
  :task (sg2 )
  :ordered-subtasks (and
    (task0 (t2g2 ))
    (task1 (xg2 ))
    (task2 (xg2 ))))
 (:method g2_s2t3_y_y
  :parameters ()
  :task (sg2 )
  :ordered-subtasks (and
    (task0 (t3g2 ))
    (task1 (yg2 ))
    (task2 (yg2 ))))
 (:action epsilon
  :parameters ())
 (:action t1g1
  :parameters ()
  :precondition (and (turna))
  :effect (and (not (turna)) (turnb) (lt1)))
 (:action t2g1
  :parameters ()
  :precondition (and (turna))
  :effect (and (not (turna)) (turnb) (lt2)))
 (:action t3g1
  :parameters ()
  :precondition (and (turna))
  :effect (and (not (turna)) (turnb) (lt3)))
 (:action xg1
  :parameters ()
  :precondition (and (turna))
  :effect (and (not (turna)) (turnb) (lx)))
 (:action yg1
  :parameters ()
  :precondition (and (turna))
  :effect (and (not (turna)) (turnb) (ly)))
 (:action t1g2
  :parameters ()
  :precondition (and (turnb) (lt1))
  :effect (and (not (turnb)) (turna) (not (lt1))))
 (:action t2g2
  :parameters ()
  :precondition (and (turnb) (lt2))
  :effect (and (not (turnb)) (turna) (not (lt2))))
 (:action t3g2
  :parameters ()
  :precondition (and (turnb) (lt3))
  :effect (and (not (turnb)) (turna) (not (lt3))))
 (:action xg2
  :parameters ()
  :precondition (and (turnb) (lx))
  :effect (and (not (turnb)) (turna) (not (lx))))
 (:action yg2
  :parameters ()
  :precondition (and (turnb) (ly))
  :effect (and (not (turnb)) (turna) (not (ly))))
)
