(define (domain strips_log_y_1-domain)
 (:requirements :strips :typing)
 (:predicates (obj ?obj - object) (truck ?truck - object) (location ?loc - object) (airplane ?airplane - object) (city ?city - object) (airport ?airport - object) (at_ ?obj - object ?loc - object) (in ?obj1 - object ?obj2 - object) (in_city ?obj - object ?city - object))
 (:action load_truck
  :parameters ( ?obj - object ?truck - object ?loc - object)
  :precondition (and (obj ?obj) (truck ?truck) (location ?loc) (at_ ?truck ?loc) (at_ ?obj ?loc))
  :effect (and (not (at_ ?obj ?loc)) (in ?obj ?truck)))
 (:action load_airplane
  :parameters ( ?obj - object ?airplane - object ?loc - object)
  :precondition (and (obj ?obj) (airplane ?airplane) (location ?loc) (at_ ?obj ?loc) (at_ ?airplane ?loc))
  :effect (and (not (at_ ?obj ?loc)) (in ?obj ?airplane)))
 (:action unload_truck
  :parameters ( ?obj - object ?truck - object ?loc - object)
  :precondition (and (obj ?obj) (truck ?truck) (location ?loc) (at_ ?truck ?loc) (in ?obj ?truck))
  :effect (and (not (in ?obj ?truck)) (at_ ?obj ?loc)))
 (:action unload_airplane
  :parameters ( ?obj - object ?airplane - object ?loc - object)
  :precondition (and (obj ?obj) (airplane ?airplane) (location ?loc) (in ?obj ?airplane) (at_ ?airplane ?loc))
  :effect (and (not (in ?obj ?airplane)) (at_ ?obj ?loc)))
 (:action drive_truck
  :parameters ( ?truck - object ?loc_from - object ?loc_to - object ?city - object)
  :precondition (and (truck ?truck) (location ?loc_from) (location ?loc_to) (city ?city) (at_ ?truck ?loc_from) (in_city ?loc_from ?city) (in_city ?loc_to ?city))
  :effect (and (not (at_ ?truck ?loc_from)) (at_ ?truck ?loc_to)))
 (:action fly_airplane
  :parameters ( ?airplane - object ?loc_from - object ?loc_to - object)
  :precondition (and (airplane ?airplane) (airport ?loc_from) (airport ?loc_to) (at_ ?airplane ?loc_from))
  :effect (and (not (at_ ?airplane ?loc_from)) (at_ ?airplane ?loc_to)))
)
