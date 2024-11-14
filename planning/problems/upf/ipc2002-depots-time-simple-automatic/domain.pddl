(define (domain depotprob1818-domain)
 (:requirements :strips :typing :durative-actions)
 (:types
    place locatable - object
    truck hoist surface - locatable
    pallet crate - surface
    depot distributor - place
 )
 (:predicates (at_ ?x - locatable ?y - place) (on ?x_0 - crate ?y_0 - surface) (in ?x_0 - crate ?y_1 - truck) (lifting ?x_1 - hoist ?y_2 - crate) (available ?x_1 - hoist) (clear ?x_2 - surface))
 (:durative-action drive
  :parameters ( ?x_3 - truck ?y - place ?z - place)
  :duration (= ?duration 10)
  :condition (and (at start (at_ ?x_3 ?y)))
  :effect (and (at start (not (at_ ?x_3 ?y))) (at end (at_ ?x_3 ?z))))
 (:durative-action lift
  :parameters ( ?x_1 - hoist ?y_2 - crate ?z_0 - surface ?p - place)
  :duration (= ?duration 1)
  :condition (and (over all (at_ ?x_1 ?p))(at start (available ?x_1))(at start (at_ ?y_2 ?p))(at start (on ?y_2 ?z_0))(at start (clear ?y_2)))
  :effect (and (at start (not (at_ ?y_2 ?p))) (at start (lifting ?x_1 ?y_2)) (at start (not (clear ?y_2))) (at start (not (available ?x_1))) (at start (clear ?z_0)) (at start (not (on ?y_2 ?z_0)))))
 (:durative-action drop
  :parameters ( ?x_1 - hoist ?y_2 - crate ?z_0 - surface ?p - place)
  :duration (= ?duration 1)
  :condition (and (over all (at_ ?x_1 ?p))(over all (at_ ?z_0 ?p))(over all (clear ?z_0))(over all (lifting ?x_1 ?y_2)))
  :effect (and (at end (available ?x_1)) (at end (not (lifting ?x_1 ?y_2))) (at end (at_ ?y_2 ?p)) (at end (not (clear ?z_0))) (at end (clear ?y_2)) (at end (on ?y_2 ?z_0))))
 (:durative-action load
  :parameters ( ?x_1 - hoist ?y_2 - crate ?z_1 - truck ?p - place)
  :duration (= ?duration 3)
  :condition (and (over all (at_ ?x_1 ?p))(over all (at_ ?z_1 ?p))(over all (lifting ?x_1 ?y_2)))
  :effect (and (at end (not (lifting ?x_1 ?y_2))) (at end (in ?y_2 ?z_1)) (at end (available ?x_1))))
 (:durative-action unload
  :parameters ( ?x_1 - hoist ?y_2 - crate ?z_1 - truck ?p - place)
  :duration (= ?duration 4)
  :condition (and (over all (at_ ?x_1 ?p))(over all (at_ ?z_1 ?p))(at start (available ?x_1))(at start (in ?y_2 ?z_1)))
  :effect (and (at start (not (in ?y_2 ?z_1))) (at start (not (available ?x_1))) (at start (lifting ?x_1 ?y_2))))
)
