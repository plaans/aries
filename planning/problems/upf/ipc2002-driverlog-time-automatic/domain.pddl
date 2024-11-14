(define (domain dlog_2_2_2-domain)
 (:requirements :strips :typing :durative-actions)
 (:types
    location locatable - object
    driver truck obj - locatable
 )
 (:predicates (at_ ?obj - locatable ?loc - location) (in ?obj1 - obj ?obj_0 - truck) (driving ?d - driver ?v - truck) (link ?x - location ?y - location) (path ?x - location ?y - location) (empty ?v - truck))
 (:functions (time_to_walk ?loc - location ?loc1 - location) (time_to_drive ?loc - location ?loc1 - location))
 (:durative-action load_truck
  :parameters ( ?obj_1 - obj ?truck - truck ?loc - location)
  :duration (= ?duration 2)
  :condition (and (over all (at_ ?truck ?loc))(at start (at_ ?obj_1 ?loc)))
  :effect (and (at start (not (at_ ?obj_1 ?loc))) (at end (in ?obj_1 ?truck))))
 (:durative-action unload_truck
  :parameters ( ?obj_1 - obj ?truck - truck ?loc - location)
  :duration (= ?duration 2)
  :condition (and (over all (at_ ?truck ?loc))(at start (in ?obj_1 ?truck)))
  :effect (and (at start (not (in ?obj_1 ?truck))) (at end (at_ ?obj_1 ?loc))))
 (:durative-action board_truck
  :parameters ( ?driver - driver ?truck - truck ?loc - location)
  :duration (= ?duration 1)
  :condition (and (over all (at_ ?truck ?loc))(at start (at_ ?driver ?loc))(at start (empty ?truck)))
  :effect (and (at start (not (at_ ?driver ?loc))) (at start (not (empty ?truck))) (at end (driving ?driver ?truck))))
 (:durative-action disembark_truck
  :parameters ( ?driver - driver ?truck - truck ?loc - location)
  :duration (= ?duration 1)
  :condition (and (over all (at_ ?truck ?loc))(at start (driving ?driver ?truck)))
  :effect (and (at start (not (driving ?driver ?truck))) (at end (at_ ?driver ?loc)) (at end (empty ?truck))))
 (:durative-action drive_truck
  :parameters ( ?truck - truck ?loc_from - location ?loc_to - location ?driver - driver)
  :duration (= ?duration (time_to_drive ?loc_from ?loc_to))
  :condition (and (at start (at_ ?truck ?loc_from))(at start (link ?loc_from ?loc_to))(over all (driving ?driver ?truck)))
  :effect (and (at start (not (at_ ?truck ?loc_from))) (at end (at_ ?truck ?loc_to))))
 (:durative-action walk
  :parameters ( ?driver - driver ?loc_from - location ?loc_to - location)
  :duration (= ?duration (time_to_walk ?loc_from ?loc_to))
  :condition (and (at start (at_ ?driver ?loc_from))(at start (path ?loc_from ?loc_to)))
  :effect (and (at start (not (at_ ?driver ?loc_from))) (at end (at_ ?driver ?loc_to))))
)
