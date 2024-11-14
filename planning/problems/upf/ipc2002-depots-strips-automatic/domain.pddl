(define (domain depotprob1818-domain)
 (:requirements :strips :typing)
 (:types
    place locatable - object
    truck hoist surface - locatable
    pallet crate - surface
    depot distributor - place
 )
 (:predicates (at_ ?x - locatable ?y - place) (on ?x_0 - crate ?y_0 - surface) (in ?x_0 - crate ?y_1 - truck) (lifting ?x_1 - hoist ?y_2 - crate) (available ?x_1 - hoist) (clear ?x_2 - surface))
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
)
