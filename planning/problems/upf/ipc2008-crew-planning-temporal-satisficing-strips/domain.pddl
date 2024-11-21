(define (domain crewplanning_1crew_1day_40utilization-domain)
 (:requirements :strips :typing :durative-actions)
 (:types
    objects_ - object
    medicalstate filterstate crewmember payloadact day exerequipment rpcm - objects_
 )
 (:predicates (mcs_finished ?ps - medicalstate ?d - day) (changed ?fs - filterstate ?d - day) (available ?c - crewmember) (done_sleep ?c - crewmember ?d - day) (done_pre_sleep ?c - crewmember ?d - day) (done_post_sleep ?c - crewmember ?d - day) (done_dpc ?c - crewmember ?d - day) (done_meal ?c - crewmember ?d - day) (done_exercise ?c - crewmember ?d - day) (done_remove_sleep_station ?ps_0 - rpcm) (done_first_reconfigure_thermal_loop ?ps_0 - rpcm) (done_replace_rpcm ?ps_0 - rpcm) (done_assemble_sleep_station ?ps_0 - rpcm) (done_second_reconfigure_thermal_loop ?ps_0 - rpcm) (done_rpcm ?ps_0 - rpcm ?d - day) (payload_act_done ?pa - payloadact) (payload_act_completed ?pa - payloadact ?d - day) (next ?d1 - day ?d2 - day) (currentday ?c - crewmember ?d - day) (initiated ?d - day) (unused ?e - exerequipment))
 (:durative-action initialize_day
  :parameters ( ?d1 - day ?d2 - day)
  :duration (= ?duration 1440)
  :condition (and (at start (initiated ?d1))(over all (next ?d1 ?d2)))
  :effect (and (at end (initiated ?d2))))
 (:durative-action post_sleep
  :parameters ( ?c - crewmember ?d1 - day ?d2 - day)
  :duration (= ?duration 195)
  :condition (and (at start (done_sleep ?c ?d1))(at start (currentday ?c ?d1))(at start (initiated ?d2))(over all (next ?d1 ?d2)))
  :effect (and (at start (not (currentday ?c ?d1))) (at end (currentday ?c ?d2)) (at end (available ?c)) (at end (done_post_sleep ?c ?d2))))
 (:durative-action have_meal
  :parameters ( ?c - crewmember ?d - day)
  :duration (= ?duration 60)
  :condition (and (at start (available ?c))(at start (done_post_sleep ?c ?d))(over all (currentday ?c ?d)))
  :effect (and (at start (not (available ?c))) (at end (available ?c)) (at end (done_meal ?c ?d))))
 (:durative-action exercise
  :parameters ( ?c - crewmember ?d - day ?e - exerequipment)
  :duration (= ?duration 60)
  :condition (and (at start (available ?c))(at start (done_post_sleep ?c ?d))(at start (unused ?e))(over all (currentday ?c ?d)))
  :effect (and (at start (not (available ?c))) (at start (not (unused ?e))) (at end (available ?c)) (at end (unused ?e)) (at end (done_exercise ?c ?d))))
 (:durative-action sleep
  :parameters ( ?c - crewmember ?d - day)
  :duration (= ?duration 600)
  :condition (and (at start (available ?c))(at start (done_exercise ?c ?d))(at start (done_meal ?c ?d))(over all (currentday ?c ?d)))
  :effect (and (at start (not (available ?c))) (at end (done_sleep ?c ?d))))
 (:durative-action change_filter
  :parameters ( ?fs - filterstate ?c - crewmember ?d - day)
  :duration (= ?duration 60)
  :condition (and (at start (available ?c))(over all (currentday ?c ?d)))
  :effect (and (at start (not (available ?c))) (at end (available ?c)) (at end (changed ?fs ?d))))
 (:durative-action medical_conference
  :parameters ( ?ps - medicalstate ?c - crewmember ?d - day)
  :duration (= ?duration 60)
  :condition (and (at start (available ?c))(over all (currentday ?c ?d)))
  :effect (and (at start (not (available ?c))) (at end (available ?c)) (at end (mcs_finished ?ps ?d))))
 (:durative-action conduct_payload_activity
  :parameters ( ?pa - payloadact ?c - crewmember)
  :duration (= ?duration 60)
  :condition (and (at start (available ?c)))
  :effect (and (at start (not (available ?c))) (at end (available ?c)) (at end (payload_act_done ?pa))))
 (:durative-action report_payload_activity_at_deadline
  :parameters ( ?pa - payloadact ?c - crewmember ?d - day)
  :duration (= ?duration 1)
  :condition (and (over all (currentday ?c ?d))(at start (payload_act_done ?pa)))
  :effect (and (at end (payload_act_completed ?pa ?d))))
 (:durative-action first_reconfigurate_thermal_loops
  :parameters ( ?ps_0 - rpcm ?c - crewmember)
  :duration (= ?duration 60)
  :condition (and (at start (available ?c)))
  :effect (and (at start (not (available ?c))) (at end (available ?c)) (at end (done_first_reconfigure_thermal_loop ?ps_0))))
 (:durative-action remove_sleep_station
  :parameters ( ?ps_0 - rpcm ?c - crewmember)
  :duration (= ?duration 60)
  :condition (and (at start (available ?c)))
  :effect (and (at start (not (available ?c))) (at end (available ?c)) (at end (done_remove_sleep_station ?ps_0))))
 (:durative-action replace_rpcm
  :parameters ( ?ps_0 - rpcm ?c - crewmember)
  :duration (= ?duration 180)
  :condition (and (at start (available ?c))(at start (done_remove_sleep_station ?ps_0))(at start (done_first_reconfigure_thermal_loop ?ps_0)))
  :effect (and (at start (not (available ?c))) (at end (available ?c)) (at end (done_replace_rpcm ?ps_0))))
 (:durative-action assemble_sleep_station
  :parameters ( ?ps_0 - rpcm ?c - crewmember)
  :duration (= ?duration 60)
  :condition (and (at start (available ?c))(at start (done_replace_rpcm ?ps_0)))
  :effect (and (at start (not (available ?c))) (at end (available ?c)) (at end (done_assemble_sleep_station ?ps_0))))
 (:durative-action second_reconfigurate_thermal_loops
  :parameters ( ?ps_0 - rpcm ?c - crewmember)
  :duration (= ?duration 60)
  :condition (and (at start (available ?c))(at start (done_replace_rpcm ?ps_0)))
  :effect (and (at start (not (available ?c))) (at end (available ?c)) (at end (done_second_reconfigure_thermal_loop ?ps_0))))
 (:durative-action finish_rpcm
  :parameters ( ?ps_0 - rpcm ?c - crewmember ?d - day)
  :duration (= ?duration 1)
  :condition (and (at start (done_assemble_sleep_station ?ps_0))(at start (done_second_reconfigure_thermal_loop ?ps_0))(over all (currentday ?c ?d)))
  :effect (and (at end (done_rpcm ?ps_0 ?d))))
)
