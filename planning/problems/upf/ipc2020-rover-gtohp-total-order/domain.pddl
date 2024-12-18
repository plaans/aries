(define (domain htn_rover_pb_01-domain)
 (:requirements :strips :typing :negative-preconditions :hierarchy :method-preconditions)
 (:types rover waypoint store camera objective mode lander)
 (:predicates (available ?x - rover) (at_ ?x - rover ?p - waypoint) (visible ?p1 - waypoint ?p2 - waypoint) (can_traverse ?x - rover ?p1 - waypoint ?p2 - waypoint) (store_of ?s - store ?x - rover) (empty ?s - store) (full ?s - store) (equipped_for_soil_analysis ?x - rover) (at_soil_sample ?p - waypoint) (have_soil_analysis ?x - rover ?p - waypoint) (equipped_for_rock_analysis ?x - rover) (at_rock_sample ?p - waypoint) (have_rock_analysis ?x - rover ?p - waypoint) (equipped_for_imaging ?x - rover) (calibration_target ?c - camera ?o - objective) (visible_from ?o - objective ?p - waypoint) (on_board ?c - camera ?x - rover) (calibrated ?c - camera ?x - rover) (supports ?c - camera ?m - mode) (have_image ?x - rover ?o - objective ?m - mode) (at_lander ?l - lander ?p - waypoint) (channel_free ?l - lander) (communicated_soil_data ?p - waypoint) (communicated_rock_data ?p - waypoint) (communicated_image_data ?o - objective ?m - mode) (visited ?p - waypoint))
 (:task do_navigate1
  :parameters ( ?x - rover ?to - waypoint))
 (:task do_navigate2
  :parameters ( ?x - rover ?from - waypoint ?to - waypoint))
 (:task empty_store
  :parameters ( ?s - store ?x - rover))
 (:task get_soil_data
  :parameters ( ?from - waypoint))
 (:task send_soil_data
  :parameters ( ?x - rover ?from - waypoint))
 (:task get_rock_data
  :parameters ( ?from - waypoint))
 (:task send_rock_data
  :parameters ( ?x - rover ?from - waypoint))
 (:task get_image_data
  :parameters ( ?o - objective ?m - mode))
 (:task send_image_data
  :parameters ( ?x - rover ?o - objective ?m - mode))
 (:task do_calibrate
  :parameters ( ?x - rover ?c - camera))
 (:method m0_do_navigate1
  :parameters ( ?x - rover ?to - waypoint)
  :task (do_navigate1 ?x ?to)
  :precondition (and (at_ ?x ?to))
  :ordered-subtasks (and
    (t1 (nop ))))
 (:method m1_do_navigate1
  :parameters ( ?x - rover ?to - waypoint ?from - waypoint)
  :task (do_navigate1 ?x ?to)
  :precondition (and (at_ ?x ?from))
  :ordered-subtasks (and
    (t1 (visit ?from))
    (t2 (do_navigate2 ?x ?from ?to))
    (t3 (unvisit ?from))))
 (:method m2_do_navigate2
  :parameters ( ?x - rover ?from - waypoint ?to - waypoint)
  :task (do_navigate2 ?x ?from ?to)
  :precondition (and (at_ ?x ?to))
  :ordered-subtasks (and
    (t1 (nop ))))
 (:method m3_do_navigate2
  :parameters ( ?x - rover ?from - waypoint ?to - waypoint)
  :task (do_navigate2 ?x ?from ?to)
  :precondition (and (can_traverse ?x ?from ?to))
  :ordered-subtasks (and
    (t1 (navigate ?x ?from ?to))))
 (:method m4_do_navigate2
  :parameters ( ?x - rover ?from - waypoint ?to - waypoint ?mid - waypoint)
  :task (do_navigate2 ?x ?from ?to)
  :precondition (and (not (can_traverse ?x ?from ?to)) (not (visited ?mid)) (can_traverse ?x ?from ?mid))
  :ordered-subtasks (and
    (t1 (navigate ?x ?from ?mid))
    (t2 (visit ?mid))
    (t3 (do_navigate2 ?x ?mid ?to))
    (t4 (unvisit ?mid))))
 (:method m5_empty_store
  :parameters ( ?s - store ?x - rover)
  :task (empty_store ?s ?x)
  :precondition (and (empty ?s))
  :ordered-subtasks (and
    (t1 (nop ))))
 (:method m6_empty_store
  :parameters ( ?s - store ?x - rover)
  :task (empty_store ?s ?x)
  :precondition (and (not (empty ?s)))
  :ordered-subtasks (and
    (t1 (drop ?x ?s))))
 (:method m7_get_soil_data
  :parameters ( ?from - waypoint ?x - rover ?s - store)
  :task (get_soil_data ?from)
  :precondition (and (store_of ?s ?x) (equipped_for_soil_analysis ?x))
  :ordered-subtasks (and
    (t1 (do_navigate1 ?x ?from))
    (t2 (empty_store ?s ?x))
    (t3 (sample_soil ?x ?s ?from))
    (t4 (send_soil_data ?x ?from))))
 (:method m8_send_soil_data
  :parameters ( ?x - rover ?from - waypoint ?l - lander ?w1 - waypoint ?w2 - waypoint)
  :task (send_soil_data ?x ?from)
  :precondition (and (at_lander ?l ?w2) (visible ?w1 ?w2))
  :ordered-subtasks (and
    (t1 (do_navigate1 ?x ?w1))
    (t2 (communicate_soil_data1 ?x ?l ?from ?w1 ?w2))))
 (:method m9_send_soil_data
  :parameters ( ?x - rover ?from - waypoint ?l - lander ?w1 - waypoint)
  :task (send_soil_data ?x ?from)
  :precondition (and (at_lander ?l ?w1) (visible ?from ?w1) (at_ ?x ?from))
  :ordered-subtasks (and
    (t1 (communicate_soil_data2 ?x ?l ?from ?w1))))
 (:method m10_get_rock_data
  :parameters ( ?from - waypoint ?x - rover ?s - store)
  :task (get_rock_data ?from)
  :precondition (and (store_of ?s ?x) (equipped_for_rock_analysis ?x))
  :ordered-subtasks (and
    (t1 (do_navigate1 ?x ?from))
    (t2 (empty_store ?s ?x))
    (t3 (sample_rock ?x ?s ?from))
    (t4 (send_rock_data ?x ?from))))
 (:method m11_send_rock_data
  :parameters ( ?x - rover ?from - waypoint ?l - lander ?w1 - waypoint ?w2 - waypoint)
  :task (send_rock_data ?x ?from)
  :precondition (and (at_lander ?l ?w2) (visible ?w1 ?w2))
  :ordered-subtasks (and
    (t1 (do_navigate1 ?x ?w1))
    (t2 (communicate_rock_data1 ?x ?l ?from ?w1 ?w2))))
 (:method m12_send_rock_data
  :parameters ( ?x - rover ?from - waypoint ?l - lander ?w1 - waypoint)
  :task (send_rock_data ?x ?from)
  :precondition (and (at_lander ?l ?w1) (visible ?from ?w1) (at_ ?x ?from))
  :ordered-subtasks (and
    (t1 (communicate_rock_data2 ?x ?l ?from ?w1))))
 (:method m13_get_image_data
  :parameters ( ?o - objective ?m - mode ?x - rover ?c - camera ?w - waypoint)
  :task (get_image_data ?o ?m)
  :precondition (and (equipped_for_imaging ?x) (on_board ?c ?x) (supports ?c ?m) (visible_from ?o ?w))
  :ordered-subtasks (and
    (t1 (do_calibrate ?x ?c))
    (t2 (do_navigate1 ?x ?w))
    (t3 (take_image ?x ?w ?o ?c ?m))
    (t4 (send_image_data ?x ?o ?m))))
 (:method m14_send_image_data
  :parameters ( ?x - rover ?o - objective ?m - mode ?l - lander ?w1 - waypoint ?w2 - waypoint)
  :task (send_image_data ?x ?o ?m)
  :precondition (and (at_lander ?l ?w2) (visible ?w1 ?w2))
  :ordered-subtasks (and
    (t1 (do_navigate1 ?x ?w1))
    (t2 (communicate_image_data ?x ?l ?o ?m ?w1 ?w2))))
 (:method m15_do_calibrate
  :parameters ( ?x - rover ?c - camera ?o - objective ?w - waypoint)
  :task (do_calibrate ?x ?c)
  :precondition (and (calibration_target ?c ?o) (visible_from ?o ?w))
  :ordered-subtasks (and
    (t1 (do_navigate1 ?x ?w))
    (t2 (calibrate ?x ?c ?o ?w))))
 (:action navigate
  :parameters ( ?x - rover ?p1 - waypoint ?p2 - waypoint)
  :precondition (and (available ?x) (at_ ?x ?p1) (can_traverse ?x ?p1 ?p2) (visible ?p1 ?p2))
  :effect (and (not (at_ ?x ?p1)) (at_ ?x ?p2)))
 (:action sample_soil
  :parameters ( ?x - rover ?s - store ?p - waypoint)
  :precondition (and (at_ ?x ?p) (at_soil_sample ?p) (equipped_for_soil_analysis ?x) (store_of ?s ?x) (empty ?s))
  :effect (and (not (empty ?s)) (not (at_soil_sample ?p)) (full ?s) (have_soil_analysis ?x ?p)))
 (:action sample_rock
  :parameters ( ?x - rover ?s - store ?p - waypoint)
  :precondition (and (at_ ?x ?p) (at_rock_sample ?p) (equipped_for_rock_analysis ?x) (store_of ?s ?x) (empty ?s))
  :effect (and (not (empty ?s)) (not (at_rock_sample ?p)) (full ?s) (have_rock_analysis ?x ?p)))
 (:action drop
  :parameters ( ?x - rover ?s - store)
  :precondition (and (store_of ?s ?x) (full ?s))
  :effect (and (not (full ?s)) (empty ?s)))
 (:action calibrate
  :parameters ( ?x - rover ?c - camera ?o - objective ?p - waypoint)
  :precondition (and (equipped_for_imaging ?x) (calibration_target ?c ?o) (at_ ?x ?p) (visible_from ?o ?p) (on_board ?c ?x))
  :effect (and (calibrated ?c ?x)))
 (:action take_image
  :parameters ( ?x - rover ?p - waypoint ?o - objective ?c - camera ?m - mode)
  :precondition (and (calibrated ?c ?x) (on_board ?c ?x) (equipped_for_imaging ?x) (supports ?c ?m) (at_ ?x ?p) (visible_from ?o ?p))
  :effect (and (not (calibrated ?c ?x)) (have_image ?x ?o ?m)))
 (:action communicate_soil_data1
  :parameters ( ?x - rover ?l - lander ?p1 - waypoint ?p2 - waypoint ?p3 - waypoint)
  :precondition (and (at_ ?x ?p2) (at_lander ?l ?p3) (have_soil_analysis ?x ?p1) (visible ?p2 ?p3) (available ?x) (channel_free ?l))
  :effect (and (communicated_soil_data ?p1) (available ?x)))
 (:action communicate_soil_data2
  :parameters ( ?x - rover ?l - lander ?p1 - waypoint ?p2 - waypoint)
  :precondition (and (at_ ?x ?p1) (at_lander ?l ?p2) (have_soil_analysis ?x ?p1) (visible ?p1 ?p2) (available ?x) (channel_free ?l))
  :effect (and (communicated_soil_data ?p1) (available ?x)))
 (:action communicate_rock_data1
  :parameters ( ?x - rover ?l - lander ?p1 - waypoint ?p2 - waypoint ?p3 - waypoint)
  :precondition (and (at_ ?x ?p2) (at_lander ?l ?p3) (have_rock_analysis ?x ?p1) (visible ?p2 ?p3) (available ?x) (channel_free ?l))
  :effect (and (communicated_rock_data ?p1) (available ?x)))
 (:action communicate_rock_data2
  :parameters ( ?x - rover ?l - lander ?p1 - waypoint ?p2 - waypoint)
  :precondition (and (at_ ?x ?p1) (at_lander ?l ?p2) (have_rock_analysis ?x ?p1) (visible ?p1 ?p2) (available ?x) (channel_free ?l))
  :effect (and (communicated_rock_data ?p1) (available ?x)))
 (:action communicate_image_data
  :parameters ( ?x - rover ?l - lander ?o - objective ?m - mode ?p1 - waypoint ?p2 - waypoint)
  :precondition (and (at_ ?x ?p1) (at_lander ?l ?p2) (have_image ?x ?o ?m) (visible ?p1 ?p2) (available ?x) (channel_free ?l))
  :effect (and (communicated_image_data ?o ?m) (available ?x) (channel_free ?l)))
 (:action visit
  :parameters ( ?p - waypoint)
  :effect (and (visited ?p)))
 (:action unvisit
  :parameters ( ?p - waypoint)
  :precondition (and (visited ?p))
  :effect (and (not (visited ?p))))
 (:action nop
  :parameters ())
)
