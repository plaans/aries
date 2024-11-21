(define (problem p1obs_1sat_1mod-problem)
 (:domain p1obs_1sat_1mod-domain)
 (:objects
   groundstation2 - calib_direction
   phenomenon4 phenomenon6 - image_direction
   instrument0 - instrument
   satellite0 - satellite
   thermograph0 - mode
 )
 (:htn
  :ordered-subtasks (and
    (task0 (do_observation phenomenon4 thermograph0))))
 (:init (on_board instrument0 satellite0) (supports instrument0 thermograph0) (calibration_target instrument0 groundstation2) (power_avail satellite0) (pointing satellite0 phenomenon6))
 (:goal (and ))
)
