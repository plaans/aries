(define (domain transport_p01_10_city_5nodes_1000size_3degree_100mindistance_2trucks_2packagespercity_2008seed-domain)
 (:requirements :strips :typing :numeric-fluents :durative-actions)
 (:types
    location target locatable - object
    vehicle package - locatable
 )
 (:predicates (road ?l1 - location ?l2 - location) (at_ ?x - locatable ?y - location) (in ?x_0 - package ?y_0 - vehicle) (has_petrol_station ?l - location) (ready_loading ?v - vehicle))
 (:functions (capacity ?v - vehicle) (road_length ?l1 - location ?l2 - location) (fuel_demand ?l1 - location ?l2 - location) (fuel_left ?v - vehicle) (fuel_max ?v - vehicle) (package_size ?p - package))
 (:durative-action drive
  :parameters ( ?v - vehicle ?l1 - location ?l2 - location)
  :duration (= ?duration (road_length ?l1 ?l2))
  :condition (and (at start (at_ ?v ?l1))(at start (road ?l1 ?l2))(at start (<= (fuel_demand ?l1 ?l2) (fuel_left ?v))))
  :effect (and (at start (not (at_ ?v ?l1))) (at start (decrease (fuel_left ?v) (fuel_demand ?l1 ?l2))) (at end (at_ ?v ?l2))))
 (:durative-action pick_up
  :parameters ( ?v - vehicle ?l - location ?p - package)
  :duration (= ?duration 1)
  :condition (and (at start (at_ ?v ?l))(at start (at_ ?p ?l))(at start (<= (package_size ?p) (capacity ?v)))(at start (ready_loading ?v))(over all (at_ ?v ?l)))
  :effect (and (at start (not (at_ ?p ?l))) (at start (decrease (capacity ?v) (package_size ?p))) (at start (not (ready_loading ?v))) (at end (in ?p ?v)) (at end (ready_loading ?v))))
 (:durative-action drop
  :parameters ( ?v - vehicle ?l - location ?p - package)
  :duration (= ?duration 1)
  :condition (and (at start (at_ ?v ?l))(at start (in ?p ?v))(at start (ready_loading ?v))(over all (at_ ?v ?l)))
  :effect (and (at start (not (in ?p ?v))) (at start (not (ready_loading ?v))) (at end (at_ ?p ?l)) (at end (increase (capacity ?v) (package_size ?p))) (at end (ready_loading ?v))))
 (:durative-action refuel
  :parameters ( ?v - vehicle ?l - location)
  :duration (= ?duration 10)
  :condition (and (at start (at_ ?v ?l))(at start (has_petrol_station ?l))(over all (at_ ?v ?l)))
  :effect (and (at end (assign (fuel_left ?v) (fuel_max ?v)))))
)
