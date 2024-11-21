(define (domain roverprob1234-domain)
 (:requirements :strips :typing)
 (:types rover waypoint store camera mode lander objective)
 (:predicates (at_ ?x - rover ?y - waypoint) (at_lander ?x_0 - lander ?y - waypoint) (can_traverse ?r - rover ?x_1 - waypoint ?y - waypoint) (equipped_for_soil_analysis ?r - rover) (equipped_for_rock_analysis ?r - rover) (equipped_for_imaging ?r - rover) (empty ?s - store) (have_rock_analysis ?r - rover ?w - waypoint) (have_soil_analysis ?r - rover ?w - waypoint) (full ?s - store) (calibrated ?c - camera ?r - rover) (supports ?c - camera ?m - mode) (available ?r - rover) (visible ?w - waypoint ?p - waypoint) (have_image ?r - rover ?o - objective ?m - mode) (communicated_soil_data ?w - waypoint) (communicated_rock_data ?w - waypoint) (communicated_image_data ?o - objective ?m - mode) (at_soil_sample ?w - waypoint) (at_rock_sample ?w - waypoint) (visible_from ?o - objective ?w - waypoint) (store_of ?s - store ?r - rover) (calibration_target ?i - camera ?o - objective) (on_board ?i - camera ?r - rover) (channel_free ?l - lander))
 (:action navigate
  :parameters ( ?x - rover ?y - waypoint ?z - waypoint)
  :precondition (and (can_traverse ?x ?y ?z) (available ?x) (at_ ?x ?y) (visible ?y ?z))
  :effect (and (not (at_ ?x ?y)) (at_ ?x ?z)))
 (:action sample_soil
  :parameters ( ?x - rover ?s - store ?p - waypoint)
  :precondition (and (at_ ?x ?p) (at_soil_sample ?p) (equipped_for_soil_analysis ?x) (store_of ?s ?x) (empty ?s))
  :effect (and (not (empty ?s)) (full ?s) (have_soil_analysis ?x ?p) (not (at_soil_sample ?p))))
 (:action sample_rock
  :parameters ( ?x - rover ?s - store ?p - waypoint)
  :precondition (and (at_ ?x ?p) (at_rock_sample ?p) (equipped_for_rock_analysis ?x) (store_of ?s ?x) (empty ?s))
  :effect (and (not (empty ?s)) (full ?s) (have_rock_analysis ?x ?p) (not (at_rock_sample ?p))))
 (:action drop
  :parameters ( ?x - rover ?y_0 - store)
  :precondition (and (store_of ?y_0 ?x) (full ?y_0))
  :effect (and (not (full ?y_0)) (empty ?y_0)))
 (:action calibrate
  :parameters ( ?r - rover ?i - camera ?t - objective ?w - waypoint)
  :precondition (and (equipped_for_imaging ?r) (calibration_target ?i ?t) (at_ ?r ?w) (visible_from ?t ?w) (on_board ?i ?r))
  :effect (and (calibrated ?i ?r)))
 (:action take_image
  :parameters ( ?r - rover ?p - waypoint ?o - objective ?i - camera ?m - mode)
  :precondition (and (calibrated ?i ?r) (on_board ?i ?r) (equipped_for_imaging ?r) (supports ?i ?m) (visible_from ?o ?p) (at_ ?r ?p))
  :effect (and (have_image ?r ?o ?m) (not (calibrated ?i ?r))))
 (:action communicate_soil_data
  :parameters ( ?r - rover ?l - lander ?p - waypoint ?x_1 - waypoint ?y - waypoint)
  :precondition (and (at_ ?r ?x_1) (at_lander ?l ?y) (have_soil_analysis ?r ?p) (visible ?x_1 ?y) (available ?r) (channel_free ?l))
  :effect (and (not (available ?r)) (not (channel_free ?l)) (channel_free ?l) (communicated_soil_data ?p) (available ?r)))
 (:action communicate_rock_data
  :parameters ( ?r - rover ?l - lander ?p - waypoint ?x_1 - waypoint ?y - waypoint)
  :precondition (and (at_ ?r ?x_1) (at_lander ?l ?y) (have_rock_analysis ?r ?p) (visible ?x_1 ?y) (available ?r) (channel_free ?l))
  :effect (and (not (available ?r)) (not (channel_free ?l)) (channel_free ?l) (communicated_rock_data ?p) (available ?r)))
 (:action communicate_image_data
  :parameters ( ?r - rover ?l - lander ?o - objective ?m - mode ?x_1 - waypoint ?y - waypoint)
  :precondition (and (at_ ?r ?x_1) (at_lander ?l ?y) (have_image ?r ?o ?m) (visible ?x_1 ?y) (available ?r) (channel_free ?l))
  :effect (and (not (available ?r)) (not (channel_free ?l)) (channel_free ?l) (communicated_image_data ?o ?m) (available ?r)))
)
