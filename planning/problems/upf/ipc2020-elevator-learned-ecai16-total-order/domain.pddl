(define (domain p-domain)
 (:requirements :strips :typing :negative-preconditions :hierarchy :method-preconditions)
 (:types passenger floor)
 (:predicates (origin ?passenger0 - passenger ?floor1 - floor) (locked_origin ?passenger0 - passenger ?floor1 - floor) (flagged_origin ?passenger0 - passenger ?floor1 - floor) (destin ?passenger0 - passenger ?floor1 - floor) (locked_destin ?passenger0 - passenger ?floor1 - floor) (flagged_destin ?passenger0 - passenger ?floor1 - floor) (above ?floor0 - floor ?floor1 - floor) (locked_above ?floor0 - floor ?floor1 - floor) (flagged_above ?floor0 - floor ?floor1 - floor) (boarded ?passenger0 - passenger) (locked_boarded ?passenger0 - passenger) (flagged_boarded ?passenger0 - passenger) (not_boarded ?passenger0 - passenger) (locked_not_boarded ?passenger0 - passenger) (flagged_not_boarded ?passenger0 - passenger) (served ?passenger0 - passenger) (locked_served ?passenger0 - passenger) (flagged_served ?passenger0 - passenger) (not_served ?passenger0 - passenger) (locked_not_served ?passenger0 - passenger) (flagged_not_served ?passenger0 - passenger) (lift_at ?floor0 - floor) (locked_lift_at ?floor0 - floor) (flagged_lift_at ?floor0 - floor))
 (:task ifunlock_boarded
  :parameters ( ?passenger0 - passenger))
 (:task ifunlock_served
  :parameters ( ?passenger0 - passenger))
 (:task ifunlock_lift_at
  :parameters ( ?floor0 - floor))
 (:task do_boarded_depart1
  :parameters ( ?floor0 - floor ?passenger1 - passenger))
 (:task do_boarded_board1
  :parameters ( ?floor0 - floor ?passenger1 - passenger))
 (:task do_served_depart2
  :parameters ( ?floor0 - floor ?passenger1 - passenger))
 (:task achieve_lift_at
  :parameters ( ?floor0 - floor))
 (:task achieve_lift_at0
  :parameters ( ?floor0 - floor))
 (:task achieve_boarded
  :parameters ( ?passenger0 - passenger))
 (:task achieve_boarded1
  :parameters ( ?passenger0 - passenger))
 (:task achieve_served
  :parameters ( ?passenger0 - passenger))
 (:task achieve_served2
  :parameters ( ?passenger0 - passenger))
 (:method ifunlock0_boarded
  :parameters ( ?passenger0 - passenger)
  :task (ifunlock_boarded ?passenger0)
  :precondition (and (flagged_boarded ?passenger0))
  :ordered-subtasks (and
    (_t196 (i_unflag_boarded ?passenger0))))
 (:method ifunlock1_boarded
  :parameters ( ?passenger0 - passenger)
  :task (ifunlock_boarded ?passenger0)
  :precondition (and (not (flagged_boarded ?passenger0)))
  :ordered-subtasks (and
    (_t197 (i_unlock_boarded ?passenger0))))
 (:method ifunlock2_served
  :parameters ( ?passenger0 - passenger)
  :task (ifunlock_served ?passenger0)
  :precondition (and (flagged_served ?passenger0))
  :ordered-subtasks (and
    (_t198 (i_unflag_served ?passenger0))))
 (:method ifunlock3_served
  :parameters ( ?passenger0 - passenger)
  :task (ifunlock_served ?passenger0)
  :precondition (and (not (flagged_served ?passenger0)))
  :ordered-subtasks (and
    (_t199 (i_unlock_served ?passenger0))))
 (:method ifunlock4_lift_at
  :parameters ( ?floor0 - floor)
  :task (ifunlock_lift_at ?floor0)
  :precondition (and (flagged_lift_at ?floor0))
  :ordered-subtasks (and
    (_t200 (i_unflag_lift_at ?floor0))))
 (:method ifunlock5_lift_at
  :parameters ( ?floor0 - floor)
  :task (ifunlock_lift_at ?floor0)
  :precondition (and (not (flagged_lift_at ?floor0)))
  :ordered-subtasks (and
    (_t201 (i_unlock_lift_at ?floor0))))
 (:method m6_do_boarded_depart1
  :parameters ( ?floor0 - floor ?passenger1 - passenger)
  :task (do_boarded_depart1 ?floor0 ?passenger1)
  :precondition (and (not (boarded ?passenger1)))
  :ordered-subtasks (and
    (_t202 (achieve_lift_at ?floor0))
    (_t203 (ifunlock_lift_at ?floor0))
    (_t204 (depart ?floor0 ?passenger1))))
 (:method m7_do_boarded_board1
  :parameters ( ?floor0 - floor ?passenger1 - passenger)
  :task (do_boarded_board1 ?floor0 ?passenger1)
  :precondition (and (not (boarded ?passenger1)))
  :ordered-subtasks (and
    (_t205 (achieve_lift_at ?floor0))
    (_t206 (ifunlock_lift_at ?floor0))
    (_t207 (board ?floor0 ?passenger1))))
 (:method m8_do_served_depart2
  :parameters ( ?floor0 - floor ?passenger1 - passenger)
  :task (do_served_depart2 ?floor0 ?passenger1)
  :precondition (and (not (served ?passenger1)))
  :ordered-subtasks (and
    (_t208 (achieve_boarded ?passenger1))
    (_t209 (achieve_lift_at ?floor0))
    (_t210 (ifunlock_lift_at ?floor0))
    (_t211 (ifunlock_boarded ?passenger1))
    (_t212 (depart ?floor0 ?passenger1))))
 (:method m9_achieve_lift_at
  :parameters ( ?floor0 - floor)
  :task (achieve_lift_at ?floor0)
  :precondition (and (locked_lift_at ?floor0))
  :ordered-subtasks (and
    (_t213 (i_flag_lift_at ?floor0))))
 (:method m10_achieve_lift_at
  :parameters ( ?floor0 - floor)
  :task (achieve_lift_at ?floor0)
  :precondition (and (lift_at ?floor0) (not (locked_lift_at ?floor0)))
  :ordered-subtasks (and
    (_t214 (i_lock_lift_at ?floor0))))
 (:method m11_achieve_lift_at
  :parameters ( ?floor0 - floor)
  :task (achieve_lift_at ?floor0)
  :precondition (and (not (locked_lift_at ?floor0)) (not (lift_at ?floor0)))
  :ordered-subtasks (and
    (_t215 (achieve_lift_at0 ?floor0))
    (_t216 (i_lock_lift_at ?floor0))))
 (:method m12_achieve_lift_at0
  :parameters ( ?floor0 - floor)
  :task (achieve_lift_at0 ?floor0)
  :precondition (and (lift_at ?floor0)))
 (:method m13_achieve_lift_at0
  :parameters ( ?floor0 - floor ?floor1 - floor ?floor3 - floor)
  :task (achieve_lift_at0 ?floor0)
  :precondition (and (not (lift_at ?floor0)) (lift_at ?floor1) (above ?floor1 ?floor3))
  :ordered-subtasks (and
    (_t217 (up ?floor1 ?floor3))
    (_t218 (achieve_lift_at0 ?floor0))))
 (:method m14_achieve_lift_at0
  :parameters ( ?floor0 - floor ?floor1 - floor ?floor3 - floor)
  :task (achieve_lift_at0 ?floor0)
  :precondition (and (not (lift_at ?floor0)) (lift_at ?floor1) (above ?floor3 ?floor1))
  :ordered-subtasks (and
    (_t219 (down ?floor1 ?floor3))
    (_t220 (achieve_lift_at0 ?floor0))))
 (:method m15_achieve_boarded
  :parameters ( ?passenger0 - passenger)
  :task (achieve_boarded ?passenger0)
  :precondition (and (locked_boarded ?passenger0))
  :ordered-subtasks (and
    (_t221 (i_flag_boarded ?passenger0))))
 (:method m16_achieve_boarded
  :parameters ( ?passenger0 - passenger)
  :task (achieve_boarded ?passenger0)
  :precondition (and (boarded ?passenger0) (not (locked_boarded ?passenger0)))
  :ordered-subtasks (and
    (_t222 (i_lock_boarded ?passenger0))))
 (:method m17_achieve_boarded
  :parameters ( ?passenger0 - passenger)
  :task (achieve_boarded ?passenger0)
  :precondition (and (not (locked_boarded ?passenger0)) (not (boarded ?passenger0)))
  :ordered-subtasks (and
    (_t223 (achieve_boarded1 ?passenger0))
    (_t224 (i_lock_boarded ?passenger0))))
 (:method m18_achieve_boarded1
  :parameters ( ?passenger0 - passenger)
  :task (achieve_boarded1 ?passenger0)
  :precondition (and (boarded ?passenger0)))
 (:method m19_achieve_boarded1
  :parameters ( ?floor2 - floor ?passenger0 - passenger)
  :task (achieve_boarded1 ?passenger0)
  :precondition (and (not (boarded ?passenger0)) (origin ?passenger0 ?floor2))
  :ordered-subtasks (and
    (_t225 (do_boarded_board1 ?floor2 ?passenger0))
    (_t226 (achieve_boarded1 ?passenger0))))
 (:method m20_achieve_served
  :parameters ( ?passenger0 - passenger)
  :task (achieve_served ?passenger0)
  :precondition (and (locked_served ?passenger0))
  :ordered-subtasks (and
    (_t227 (i_flag_served ?passenger0))))
 (:method m21_achieve_served
  :parameters ( ?passenger0 - passenger)
  :task (achieve_served ?passenger0)
  :precondition (and (served ?passenger0) (not (locked_served ?passenger0)))
  :ordered-subtasks (and
    (_t228 (i_lock_served ?passenger0))))
 (:method m22_achieve_served
  :parameters ( ?passenger0 - passenger)
  :task (achieve_served ?passenger0)
  :precondition (and (not (locked_served ?passenger0)) (not (served ?passenger0)))
  :ordered-subtasks (and
    (_t229 (achieve_served2 ?passenger0))
    (_t230 (i_lock_served ?passenger0))))
 (:method m23_achieve_served2
  :parameters ( ?passenger0 - passenger)
  :task (achieve_served2 ?passenger0)
  :precondition (and (served ?passenger0)))
 (:method m24_achieve_served2
  :parameters ( ?floor2 - floor ?passenger0 - passenger)
  :task (achieve_served2 ?passenger0)
  :precondition (and (not (served ?passenger0)) (destin ?passenger0 ?floor2))
  :ordered-subtasks (and
    (_t231 (do_served_depart2 ?floor2 ?passenger0))
    (_t232 (achieve_served2 ?passenger0))))
 (:action board
  :parameters ( ?floor0 - floor ?passenger1 - passenger)
  :precondition (and (lift_at ?floor0) (origin ?passenger1 ?floor0))
  :effect (and (boarded ?passenger1)))
 (:action depart
  :parameters ( ?floor0 - floor ?passenger1 - passenger)
  :precondition (and (lift_at ?floor0) (destin ?passenger1 ?floor0) (boarded ?passenger1) (not (locked_boarded ?passenger1)))
  :effect (and (not (boarded ?passenger1)) (served ?passenger1)))
 (:action up
  :parameters ( ?floor0 - floor ?floor1 - floor)
  :precondition (and (lift_at ?floor0) (above ?floor0 ?floor1) (not (locked_lift_at ?floor0)))
  :effect (and (not (lift_at ?floor0)) (lift_at ?floor1)))
 (:action down
  :parameters ( ?floor0 - floor ?floor1 - floor)
  :precondition (and (lift_at ?floor0) (above ?floor1 ?floor0) (not (locked_lift_at ?floor0)))
  :effect (and (not (lift_at ?floor0)) (lift_at ?floor1)))
 (:action i_lock_boarded
  :parameters ( ?passenger0 - passenger)
  :effect (and (locked_boarded ?passenger0)))
 (:action i_unlock_boarded
  :parameters ( ?passenger0 - passenger)
  :effect (and (not (locked_boarded ?passenger0))))
 (:action i_flag_boarded
  :parameters ( ?passenger0 - passenger)
  :effect (and (flagged_boarded ?passenger0)))
 (:action i_unflag_boarded
  :parameters ( ?passenger0 - passenger)
  :effect (and (not (flagged_boarded ?passenger0))))
 (:action i_lock_served
  :parameters ( ?passenger0 - passenger)
  :effect (and (locked_served ?passenger0)))
 (:action i_unlock_served
  :parameters ( ?passenger0 - passenger)
  :effect (and (not (locked_served ?passenger0))))
 (:action i_flag_served
  :parameters ( ?passenger0 - passenger)
  :effect (and (flagged_served ?passenger0)))
 (:action i_unflag_served
  :parameters ( ?passenger0 - passenger)
  :effect (and (not (flagged_served ?passenger0))))
 (:action i_lock_lift_at
  :parameters ( ?floor0 - floor)
  :effect (and (locked_lift_at ?floor0)))
 (:action i_unlock_lift_at
  :parameters ( ?floor0 - floor)
  :effect (and (not (locked_lift_at ?floor0))))
 (:action i_flag_lift_at
  :parameters ( ?floor0 - floor)
  :effect (and (flagged_lift_at ?floor0)))
 (:action i_unflag_lift_at
  :parameters ( ?floor0 - floor)
  :effect (and (not (flagged_lift_at ?floor0))))
)
