(define (domain depotprob1818-domain)
 (:requirements :strips :typing :negative-preconditions :hierarchy :method-preconditions)
 (:types
    place locatable - object
    truck hoist surface - locatable
    pallet crate - surface
    depot distributor - place
 )
 (:predicates (at_ ?x - locatable ?y - place) (on ?x_0 - crate ?y_0 - surface) (in ?x_0 - crate ?y_1 - truck) (lifting ?x_1 - hoist ?y_2 - crate) (available ?x_1 - hoist) (clear ?x_2 - surface))
 (:task do_put_on
  :parameters ( ?c - crate ?s2 - surface))
 (:task do_clear
  :parameters ( ?s1 - surface ?p1 - place))
 (:task do_get_truck
  :parameters ( ?t - truck ?p1 - place))
 (:task do_lift_crate
  :parameters ( ?c - crate ?p - place ?h - hoist))
 (:task do_load_truck
  :parameters ( ?c - crate ?s - surface ?p - place ?t - truck))
 (:task do_unload_truck
  :parameters ( ?c - crate ?s - surface ?p - place ?t - truck))
 (:method m0_do_put_on
  :parameters ( ?c - crate ?s2 - surface)
  :task (do_put_on ?c ?s2)
  :precondition (and (on ?c ?s2))
  :ordered-subtasks (and
    (t1 (nop ))))
 (:method m1_do_put_on
  :parameters ( ?c - crate ?s2 - surface ?p - place ?h - hoist)
  :task (do_put_on ?c ?s2)
  :precondition (and (at_ ?c ?p))
  :ordered-subtasks (and
    (t1 (do_clear ?c ?p))
    (t2 (do_clear ?s2 ?p))
    (t3 (do_lift_crate ?c ?p ?h))
    (t4 (drop ?h ?c ?s2 ?p))))
 (:method m2_do_put_on
  :parameters ( ?c - crate ?s2 - surface ?p - place ?t - truck ?h - hoist)
  :task (do_put_on ?c ?s2)
  :precondition (and (in ?c ?t))
  :ordered-subtasks (and
    (t1 (do_get_truck ?t ?p))
    (t2 (do_clear ?s2 ?p))
    (t3 (unload ?h ?c ?t ?p))
    (t4 (drop ?h ?c ?s2 ?p))))
 (:method m3_do_put_on
  :parameters ( ?c - crate ?s2 - surface ?s1 - surface ?p1 - place ?t - truck ?p2 - place)
  :task (do_put_on ?c ?s2)
  :ordered-subtasks (and
    (t1 (do_load_truck ?c ?s1 ?p1 ?t))
    (t2 (drive ?t ?p1 ?p2))
    (t3 (do_unload_truck ?c ?s2 ?p2 ?t))))
 (:method m4_do_clear
  :parameters ( ?s1 - surface ?p1 - place)
  :task (do_clear ?s1 ?p1)
  :precondition (and (clear ?s1) (at_ ?s1 ?p1))
  :ordered-subtasks (and
    (t1 (nop ))))
 (:method m5_do_clear
  :parameters ( ?s1 - surface ?p1 - place ?c - crate ?t - truck ?h1 - hoist)
  :task (do_clear ?s1 ?p1)
  :precondition (and (not (clear ?s1)) (on ?c ?s1) (at_ ?s1 ?p1) (at_ ?h1 ?p1))
  :ordered-subtasks (and
    (t1 (do_clear ?c ?p1))
    (t2 (lift ?h1 ?c ?s1 ?p1))
    (t3 (do_get_truck ?t ?p1))
    (t4 (load ?h1 ?c ?t ?p1))))
 (:method m6_do_get_truck
  :parameters ( ?t - truck ?p1 - place)
  :task (do_get_truck ?t ?p1)
  :precondition (and (at_ ?t ?p1))
  :ordered-subtasks (and
    (t1 (nop ))))
 (:method m7_do_get_truck
  :parameters ( ?t - truck ?p1 - place ?p2 - place)
  :task (do_get_truck ?t ?p1)
  :precondition (and (not (at_ ?t ?p1)))
  :ordered-subtasks (and
    (t1 (drive ?t ?p2 ?p1))))
 (:method m8_do_lift_crate
  :parameters ( ?c - crate ?p - place ?h - hoist ?t - truck)
  :task (do_lift_crate ?c ?p ?h)
  :precondition (and (in ?c ?t) (at_ ?h ?p))
  :ordered-subtasks (and
    (t1 (do_get_truck ?t ?p))
    (t2 (unload ?h ?c ?t ?p))))
 (:method m9_do_lift_crate
  :parameters ( ?c - crate ?p - place ?h - hoist ?s - surface)
  :task (do_lift_crate ?c ?p ?h)
  :precondition (and (on ?c ?s) (at_ ?c ?p) (at_ ?s ?p) (at_ ?h ?p))
  :ordered-subtasks (and
    (t1 (lift ?h ?c ?s ?p))))
 (:method m10_do_load_truck
  :parameters ( ?c - crate ?s - surface ?p - place ?t - truck ?h - hoist)
  :task (do_load_truck ?c ?s ?p ?t)
  :precondition (and (at_ ?c ?p) (at_ ?s ?p) (on ?c ?s) (at_ ?h ?p))
  :ordered-subtasks (and
    (t1 (do_get_truck ?t ?p))
    (t2 (do_clear ?c ?p))
    (t3 (lift ?h ?c ?s ?p))
    (t4 (load ?h ?c ?t ?p))))
 (:method m11_do_unload_truck
  :parameters ( ?c - crate ?s - surface ?p - place ?t - truck ?h - hoist)
  :task (do_unload_truck ?c ?s ?p ?t)
  :precondition (and (in ?c ?t) (at_ ?t ?p) (at_ ?h ?p) (at_ ?s ?p))
  :ordered-subtasks (and
    (t1 (do_clear ?s ?p))
    (t2 (unload ?h ?c ?t ?p))
    (t3 (drop ?h ?c ?s ?p))))
 (:action drive
  :parameters ( ?x_3 - truck ?y - place ?z - place)
  :precondition (and (at_ ?x_3 ?y))
  :effect (and (not (at_ ?x_3 ?y)) (at_ ?x_3 ?z)))
 (:action lift
  :parameters ( ?x_1 - hoist ?y_2 - crate ?z_0 - surface ?p - place)
  :precondition (and (at_ ?x_1 ?p) (available ?x_1) (at_ ?y_2 ?p) (on ?y_2 ?z_0) (clear ?y_2))
  :effect (and (not (at_ ?y_2 ?p)) (lifting ?x_1 ?y_2) (not (clear ?y_2)) (not (available ?x_1)) (clear ?z_0) (not (on ?y_2 ?z_0))))
 (:action drop
  :parameters ( ?x_1 - hoist ?y_2 - crate ?z_0 - surface ?p - place)
  :precondition (and (at_ ?x_1 ?p) (at_ ?z_0 ?p) (clear ?z_0) (lifting ?x_1 ?y_2))
  :effect (and (available ?x_1) (not (lifting ?x_1 ?y_2)) (at_ ?y_2 ?p) (not (clear ?z_0)) (clear ?y_2) (on ?y_2 ?z_0)))
 (:action load
  :parameters ( ?x_1 - hoist ?y_2 - crate ?z_1 - truck ?p - place)
  :precondition (and (at_ ?x_1 ?p) (at_ ?z_1 ?p) (lifting ?x_1 ?y_2))
  :effect (and (not (lifting ?x_1 ?y_2)) (in ?y_2 ?z_1) (available ?x_1)))
 (:action unload
  :parameters ( ?x_1 - hoist ?y_2 - crate ?z_1 - truck ?p - place)
  :precondition (and (at_ ?x_1 ?p) (at_ ?z_1 ?p) (available ?x_1) (in ?y_2 ?z_1))
  :effect (and (not (in ?y_2 ?z_1)) (not (available ?x_1)) (lifting ?x_1 ?y_2)))
 (:action nop
  :parameters ())
)
