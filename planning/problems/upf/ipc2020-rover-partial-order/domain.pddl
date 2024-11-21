(define (domain roverprob1234-domain)
 (:requirements :strips :typing :negative-preconditions :hierarchy :method-preconditions)
 (:types rover waypoint objective camera mode lander store)
 (:predicates (at_ ?rover - rover ?wp - waypoint) (at_lander ?l - lander ?y - waypoint) (at_rock_sample ?p - waypoint) (at_soil_sample ?p - waypoint) (available ?x - rover) (calibrated ?i - camera ?r - rover) (calibration_target ?camera - camera ?objective - objective) (can_traverse ?rover - rover ?from - waypoint ?to - waypoint) (channel_free ?l - lander) (communicated_image_data ?o - objective ?m - mode) (communicated_rock_data ?p - waypoint) (communicated_soil_data ?p - waypoint) (empty ?s - store) (equipped_for_imaging ?rover - rover) (equipped_for_rock_analysis ?rover - rover) (equipped_for_soil_analysis ?rover - rover) (full ?s - store) (have_image ?r - rover ?o - objective ?m - mode) (have_rock_analysis ?x - rover ?p - waypoint) (have_soil_analysis ?x - rover ?p - waypoint) (on_board ?camera - camera ?rover - rover) (store_of ?s - store ?rover - rover) (supports ?camera - camera ?mode - mode) (visible ?x_0 - waypoint ?y - waypoint) (visible_from ?objective - objective ?waypoint - waypoint) (visited ?mid - waypoint))
 (:task calibrate_abs
  :parameters ( ?rover - rover ?camera - camera))
 (:task empty_store
  :parameters ( ?s - store ?rover - rover))
 (:task get_image_data
  :parameters ( ?objective - objective ?mode - mode))
 (:task get_rock_data
  :parameters ( ?waypoint - waypoint))
 (:task get_soil_data
  :parameters ( ?waypoint - waypoint))
 (:task navigate_abs
  :parameters ( ?rover - rover ?to - waypoint))
 (:task send_image_data
  :parameters ( ?rover - rover ?objective - objective ?mode - mode))
 (:task send_rock_data
  :parameters ( ?rover - rover ?waypoint - waypoint))
 (:task send_soil_data
  :parameters ( ?rover - rover ?waypoint - waypoint))
 (:method m_empty_store_1
  :parameters ( ?s - store ?rover - rover)
  :task (empty_store ?s ?rover)
  :precondition (and (empty ?s)))
 (:method m_empty_store_2
  :parameters ( ?s - store ?rover - rover)
  :task (empty_store ?s ?rover)
  :precondition (and (not (empty ?s)))
  :ordered-subtasks (and
    (_t1766 (drop ?rover ?s))))
 (:method m_navigate_abs_1
  :parameters ( ?rover - rover ?from - waypoint ?to - waypoint)
  :task (navigate_abs ?rover ?to)
  :precondition (and (at_ ?rover ?from))
  :ordered-subtasks (and
    (_t1767 (visit ?from))
    (_t1768 (navigate ?rover ?from ?to))
    (_t1769 (unvisit ?from))))
 (:method m_navigate_abs_2
  :parameters ( ?rover - rover ?to - waypoint)
  :task (navigate_abs ?rover ?to)
  :precondition (and (at_ ?rover ?to)))
 (:method m_navigate_abs_3
  :parameters ( ?rover - rover ?from - waypoint ?to - waypoint)
  :task (navigate_abs ?rover ?to)
  :precondition (and (not (at_ ?rover ?to)) (can_traverse ?rover ?from ?to))
  :ordered-subtasks (and
    (_t1770 (navigate ?rover ?from ?to))))
 (:method m_navigate_abs_4
  :parameters ( ?rover - rover ?from - waypoint ?to - waypoint ?mid - waypoint)
  :task (navigate_abs ?rover ?to)
  :precondition (and (not (at_ ?rover ?to)) (not (can_traverse ?rover ?from ?to)) (can_traverse ?rover ?from ?mid) (not (visited ?mid)))
  :ordered-subtasks (and
    (_t1771 (navigate ?rover ?from ?mid))
    (_t1772 (visit ?mid))
    (_t1773 (navigate ?rover ?mid ?to))
    (_t1774 (unvisit ?mid))))
 (:method m_send_soil_data
  :parameters ( ?rover - rover ?waypoint - waypoint ?x_0 - waypoint ?y - waypoint ?l - lander)
  :task (send_soil_data ?rover ?waypoint)
  :precondition (and (at_lander ?l ?y) (visible ?x_0 ?y))
  :ordered-subtasks (and
    (_t1775 (navigate_abs ?rover ?x_0))
    (_t1776 (communicate_soil_data ?rover ?l ?waypoint ?x_0 ?y))))
 (:method m_get_soil_data
  :parameters ( ?waypoint - waypoint ?rover - rover ?s - store)
  :task (get_soil_data ?waypoint)
  :precondition (and (store_of ?s ?rover) (equipped_for_soil_analysis ?rover))
  :ordered-subtasks (and
    (_t1777 (navigate_abs ?rover ?waypoint))
    (_t1778 (empty_store ?s ?rover))
    (_t1779 (sample_soil ?rover ?s ?waypoint))
    (_t1780 (send_soil_data ?rover ?waypoint))))
 (:method m_send_rock_data
  :parameters ( ?rover - rover ?waypoint - waypoint ?x_0 - waypoint ?y - waypoint ?l - lander)
  :task (send_rock_data ?rover ?waypoint)
  :precondition (and (at_lander ?l ?y) (visible ?x_0 ?y))
  :ordered-subtasks (and
    (_t1781 (navigate_abs ?rover ?x_0))
    (_t1782 (communicate_rock_data ?rover ?l ?waypoint ?x_0 ?y))))
 (:method m_get_rock_data
  :parameters ( ?waypoint - waypoint ?rover - rover ?s - store)
  :task (get_rock_data ?waypoint)
  :precondition (and (equipped_for_rock_analysis ?rover) (store_of ?s ?rover))
  :ordered-subtasks (and
    (_t1783 (navigate_abs ?rover ?waypoint))
    (_t1784 (empty_store ?s ?rover))
    (_t1785 (sample_rock ?rover ?s ?waypoint))
    (_t1786 (send_rock_data ?rover ?waypoint))))
 (:method m_send_image_data
  :parameters ( ?rover - rover ?objective - objective ?mode - mode ?x_0 - waypoint ?y - waypoint ?l - lander)
  :task (send_image_data ?rover ?objective ?mode)
  :precondition (and (at_lander ?l ?y) (visible ?x_0 ?y))
  :ordered-subtasks (and
    (_t1787 (navigate_abs ?rover ?x_0))
    (_t1788 (communicate_image_data ?rover ?l ?objective ?mode ?x_0 ?y))))
 (:method m_get_image_data
  :parameters ( ?objective - objective ?mode - mode ?camera - camera ?rover - rover ?waypoint - waypoint)
  :task (get_image_data ?objective ?mode)
  :precondition (and (equipped_for_imaging ?rover) (on_board ?camera ?rover) (supports ?camera ?mode) (visible_from ?objective ?waypoint))
  :ordered-subtasks (and
    (_t1789 (calibrate_abs ?rover ?camera))
    (_t1790 (navigate_abs ?rover ?waypoint))
    (_t1791 (take_image ?rover ?waypoint ?objective ?camera ?mode))
    (_t1792 (send_image_data ?rover ?objective ?mode))))
 (:method m_calibrate_abs
  :parameters ( ?rover - rover ?camera - camera ?objective - objective ?waypoint - waypoint)
  :task (calibrate_abs ?rover ?camera)
  :precondition (and (calibration_target ?camera ?objective) (visible_from ?objective ?waypoint))
  :ordered-subtasks (and
    (_t1793 (navigate_abs ?rover ?waypoint))
    (_t1794 (calibrate ?rover ?camera ?objective ?waypoint))))
 (:action navigate
  :parameters ( ?x - rover ?y - waypoint ?z - waypoint)
  :precondition (and (can_traverse ?x ?y ?z) (available ?x) (at_ ?x ?y) (visible ?y ?z))
  :effect (and (not (at_ ?x ?y)) (at_ ?x ?z)))
 (:action sample_soil
  :parameters ( ?x - rover ?s - store ?p - waypoint)
  :precondition (and (at_ ?x ?p) (at_soil_sample ?p) (equipped_for_soil_analysis ?x) (store_of ?s ?x) (empty ?s))
  :effect (and (not (empty ?s)) (not (at_soil_sample ?p)) (full ?s) (have_soil_analysis ?x ?p)))
 (:action sample_rock
  :parameters ( ?x - rover ?s - store ?p - waypoint)
  :precondition (and (at_rock_sample ?p) (equipped_for_rock_analysis ?x) (store_of ?s ?x) (empty ?s))
  :effect (and (not (empty ?s)) (not (at_rock_sample ?p)) (full ?s) (have_rock_analysis ?x ?p)))
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
  :effect (and (not (calibrated ?i ?r)) (have_image ?r ?o ?m)))
 (:action communicate_soil_data
  :parameters ( ?r - rover ?l - lander ?p - waypoint ?x_0 - waypoint ?y - waypoint)
  :precondition (and (at_ ?r ?x_0) (at_lander ?l ?y) (have_soil_analysis ?r ?p) (visible ?x_0 ?y) (available ?r) (channel_free ?l))
  :effect (and (channel_free ?l) (communicated_soil_data ?p) (available ?r)))
 (:action communicate_rock_data
  :parameters ( ?r - rover ?l - lander ?p - waypoint ?x_0 - waypoint ?y - waypoint)
  :precondition (and (at_ ?r ?x_0) (at_lander ?l ?y) (have_rock_analysis ?r ?p) (visible ?x_0 ?y) (available ?r) (channel_free ?l))
  :effect (and (channel_free ?l) (communicated_rock_data ?p) (available ?r)))
 (:action communicate_image_data
  :parameters ( ?r - rover ?l - lander ?o - objective ?m - mode ?x_0 - waypoint ?y - waypoint)
  :precondition (and (at_ ?r ?x_0) (at_lander ?l ?y) (have_image ?r ?o ?m) (visible ?x_0 ?y) (available ?r) (channel_free ?l))
  :effect (and (channel_free ?l) (communicated_image_data ?o ?m) (available ?r)))
 (:action visit
  :parameters ( ?waypoint - waypoint)
  :effect (and (visited ?waypoint)))
 (:action unvisit
  :parameters ( ?waypoint - waypoint)
  :effect (and (not (visited ?waypoint))))
)
