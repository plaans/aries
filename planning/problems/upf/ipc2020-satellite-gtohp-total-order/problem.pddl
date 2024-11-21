(define (problem strips_sat_x_1-problem)
 (:domain strips_sat_x_1-domain)
 (:objects
   satellite0 - satellite
   star0 groundstation1 groundstation2 phenomenon3 phenomenon4 star5 phenomenon6 - direction
   instrument0 - instrument
   image1 spectrograph2 thermograph0 - mode
 )
 (:htn
  :ordered-subtasks (and
    (task1 (do_mission phenomenon4 thermograph0))
    (task2 (do_mission star5 thermograph0))
    (task3 (do_mission phenomenon6 thermograph0))))
 (:init (supports instrument0 thermograph0) (calibration_target instrument0 groundstation2) (on_board instrument0 satellite0) (power_avail satellite0) (pointing satellite0 phenomenon6))
 (:goal (and (have_image phenomenon4 thermograph0) (have_image star5 thermograph0) (have_image phenomenon6 thermograph0)))
)
