(define (domain grounded_strips_roverprob1234-domain)
 (:requirements :strips)
 (:predicates (at_rover0_waypoint1) (at_rover0_waypoint0) (at_rover0_waypoint2) (full_rover0store) (have_soil_analysis_rover0_waypoint0) (have_soil_analysis_rover0_waypoint2) (have_soil_analysis_rover0_waypoint3) (have_rock_analysis_rover0_waypoint1) (have_rock_analysis_rover0_waypoint2) (have_rock_analysis_rover0_waypoint3) (calibrated_camera0_rover0) (have_image_rover0_objective1_high_res) (have_image_rover0_objective1_colour) (have_image_rover0_objective0_high_res) (have_image_rover0_objective0_colour) (communicated_soil_data_waypoint0) (communicated_soil_data_waypoint2) (communicated_soil_data_waypoint3) (communicated_rock_data_waypoint1) (communicated_rock_data_waypoint2) (communicated_rock_data_waypoint3) (communicated_image_data_objective0_colour) (communicated_image_data_objective0_high_res) (communicated_image_data_objective1_colour) (communicated_image_data_objective1_high_res) (available_rover0) (channel_free_general) (empty_rover0store) (at_rock_sample_waypoint3) (at_rock_sample_waypoint2) (at_rock_sample_waypoint1) (at_soil_sample_waypoint3) (at_soil_sample_waypoint2) (at_soil_sample_waypoint0) (at_rover0_waypoint3))
 (:action navigate_rover0_waypoint0_waypoint3
  :parameters ()
  :precondition (and (at_rover0_waypoint0) (available_rover0))
  :effect (and (at_rover0_waypoint3) (not (at_rover0_waypoint0))))
 (:action navigate_rover0_waypoint1_waypoint3
  :parameters ()
  :precondition (and (at_rover0_waypoint1) (available_rover0))
  :effect (and (at_rover0_waypoint3) (not (at_rover0_waypoint1))))
 (:action communicate_image_data_rover0_general_objective1_high_res_waypoint1_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective1_high_res) (at_rover0_waypoint1))
  :effect (and (channel_free_general) (communicated_image_data_objective1_high_res) (available_rover0)))
 (:action communicate_image_data_rover0_general_objective1_colour_waypoint1_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective1_colour) (at_rover0_waypoint1))
  :effect (and (channel_free_general) (communicated_image_data_objective1_colour) (available_rover0)))
 (:action communicate_image_data_rover0_general_objective0_high_res_waypoint1_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective0_high_res) (at_rover0_waypoint1))
  :effect (and (channel_free_general) (communicated_image_data_objective0_high_res) (available_rover0)))
 (:action communicate_image_data_rover0_general_objective0_colour_waypoint1_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective0_colour) (at_rover0_waypoint1))
  :effect (and (channel_free_general) (communicated_image_data_objective0_colour) (available_rover0)))
 (:action communicate_image_data_rover0_general_objective1_high_res_waypoint2_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective1_high_res) (at_rover0_waypoint2))
  :effect (and (channel_free_general) (communicated_image_data_objective1_high_res) (available_rover0)))
 (:action communicate_image_data_rover0_general_objective1_colour_waypoint2_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective1_colour) (at_rover0_waypoint2))
  :effect (and (channel_free_general) (communicated_image_data_objective1_colour) (available_rover0)))
 (:action communicate_image_data_rover0_general_objective0_high_res_waypoint2_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective0_high_res) (at_rover0_waypoint2))
  :effect (and (channel_free_general) (communicated_image_data_objective0_high_res) (available_rover0)))
 (:action communicate_image_data_rover0_general_objective0_colour_waypoint2_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective0_colour) (at_rover0_waypoint2))
  :effect (and (channel_free_general) (communicated_image_data_objective0_colour) (available_rover0)))
 (:action communicate_image_data_rover0_general_objective1_high_res_waypoint3_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective1_high_res) (at_rover0_waypoint3))
  :effect (and (channel_free_general) (communicated_image_data_objective1_high_res) (available_rover0)))
 (:action communicate_image_data_rover0_general_objective1_colour_waypoint3_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective1_colour) (at_rover0_waypoint3))
  :effect (and (channel_free_general) (communicated_image_data_objective1_colour) (available_rover0)))
 (:action communicate_image_data_rover0_general_objective0_high_res_waypoint3_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective0_high_res) (at_rover0_waypoint3))
  :effect (and (channel_free_general) (communicated_image_data_objective0_high_res) (available_rover0)))
 (:action communicate_image_data_rover0_general_objective0_colour_waypoint3_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_image_rover0_objective0_colour) (at_rover0_waypoint3))
  :effect (and (channel_free_general) (communicated_image_data_objective0_colour) (available_rover0)))
 (:action communicate_rock_data_rover0_general_waypoint3_waypoint1_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_rock_analysis_rover0_waypoint3) (at_rover0_waypoint1))
  :effect (and (channel_free_general) (communicated_rock_data_waypoint3) (available_rover0)))
 (:action communicate_rock_data_rover0_general_waypoint2_waypoint1_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_rock_analysis_rover0_waypoint2) (at_rover0_waypoint1))
  :effect (and (channel_free_general) (communicated_rock_data_waypoint2) (available_rover0)))
 (:action communicate_rock_data_rover0_general_waypoint1_waypoint1_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_rock_analysis_rover0_waypoint1) (at_rover0_waypoint1))
  :effect (and (channel_free_general) (communicated_rock_data_waypoint1) (available_rover0)))
 (:action communicate_rock_data_rover0_general_waypoint3_waypoint2_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_rock_analysis_rover0_waypoint3) (at_rover0_waypoint2))
  :effect (and (channel_free_general) (communicated_rock_data_waypoint3) (available_rover0)))
 (:action communicate_rock_data_rover0_general_waypoint2_waypoint2_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_rock_analysis_rover0_waypoint2) (at_rover0_waypoint2))
  :effect (and (channel_free_general) (communicated_rock_data_waypoint2) (available_rover0)))
 (:action communicate_rock_data_rover0_general_waypoint1_waypoint2_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_rock_analysis_rover0_waypoint1) (at_rover0_waypoint2))
  :effect (and (channel_free_general) (communicated_rock_data_waypoint1) (available_rover0)))
 (:action communicate_rock_data_rover0_general_waypoint3_waypoint3_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_rock_analysis_rover0_waypoint3) (at_rover0_waypoint3))
  :effect (and (channel_free_general) (communicated_rock_data_waypoint3) (available_rover0)))
 (:action communicate_rock_data_rover0_general_waypoint2_waypoint3_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_rock_analysis_rover0_waypoint2) (at_rover0_waypoint3))
  :effect (and (channel_free_general) (communicated_rock_data_waypoint2) (available_rover0)))
 (:action communicate_rock_data_rover0_general_waypoint1_waypoint3_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_rock_analysis_rover0_waypoint1) (at_rover0_waypoint3))
  :effect (and (channel_free_general) (communicated_rock_data_waypoint1) (available_rover0)))
 (:action communicate_soil_data_rover0_general_waypoint3_waypoint1_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_soil_analysis_rover0_waypoint3) (at_rover0_waypoint1))
  :effect (and (channel_free_general) (communicated_soil_data_waypoint3) (available_rover0)))
 (:action communicate_soil_data_rover0_general_waypoint2_waypoint1_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_soil_analysis_rover0_waypoint2) (at_rover0_waypoint1))
  :effect (and (channel_free_general) (communicated_soil_data_waypoint2) (available_rover0)))
 (:action communicate_soil_data_rover0_general_waypoint0_waypoint1_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_soil_analysis_rover0_waypoint0) (at_rover0_waypoint1))
  :effect (and (channel_free_general) (communicated_soil_data_waypoint0) (available_rover0)))
 (:action communicate_soil_data_rover0_general_waypoint3_waypoint2_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_soil_analysis_rover0_waypoint3) (at_rover0_waypoint2))
  :effect (and (channel_free_general) (communicated_soil_data_waypoint3) (available_rover0)))
 (:action communicate_soil_data_rover0_general_waypoint2_waypoint2_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_soil_analysis_rover0_waypoint2) (at_rover0_waypoint2))
  :effect (and (channel_free_general) (communicated_soil_data_waypoint2) (available_rover0)))
 (:action communicate_soil_data_rover0_general_waypoint0_waypoint2_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_soil_analysis_rover0_waypoint0) (at_rover0_waypoint2))
  :effect (and (channel_free_general) (communicated_soil_data_waypoint0) (available_rover0)))
 (:action communicate_soil_data_rover0_general_waypoint3_waypoint3_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_soil_analysis_rover0_waypoint3) (at_rover0_waypoint3))
  :effect (and (channel_free_general) (communicated_soil_data_waypoint3) (available_rover0)))
 (:action communicate_soil_data_rover0_general_waypoint2_waypoint3_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_soil_analysis_rover0_waypoint2) (at_rover0_waypoint3))
  :effect (and (channel_free_general) (communicated_soil_data_waypoint2) (available_rover0)))
 (:action communicate_soil_data_rover0_general_waypoint0_waypoint3_waypoint0
  :parameters ()
  :precondition (and (channel_free_general) (available_rover0) (have_soil_analysis_rover0_waypoint0) (at_rover0_waypoint3))
  :effect (and (channel_free_general) (communicated_soil_data_waypoint0) (available_rover0)))
 (:action take_image_rover0_waypoint0_objective0_camera0_colour
  :parameters ()
  :precondition (and (at_rover0_waypoint0) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective0_colour) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint0_objective0_camera0_high_res
  :parameters ()
  :precondition (and (at_rover0_waypoint0) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective0_high_res) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint1_objective0_camera0_colour
  :parameters ()
  :precondition (and (at_rover0_waypoint1) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective0_colour) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint1_objective0_camera0_high_res
  :parameters ()
  :precondition (and (at_rover0_waypoint1) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective0_high_res) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint2_objective0_camera0_colour
  :parameters ()
  :precondition (and (at_rover0_waypoint2) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective0_colour) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint2_objective0_camera0_high_res
  :parameters ()
  :precondition (and (at_rover0_waypoint2) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective0_high_res) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint3_objective0_camera0_colour
  :parameters ()
  :precondition (and (at_rover0_waypoint3) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective0_colour) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint3_objective0_camera0_high_res
  :parameters ()
  :precondition (and (at_rover0_waypoint3) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective0_high_res) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint0_objective1_camera0_colour
  :parameters ()
  :precondition (and (at_rover0_waypoint0) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective1_colour) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint0_objective1_camera0_high_res
  :parameters ()
  :precondition (and (at_rover0_waypoint0) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective1_high_res) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint1_objective1_camera0_colour
  :parameters ()
  :precondition (and (at_rover0_waypoint1) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective1_colour) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint1_objective1_camera0_high_res
  :parameters ()
  :precondition (and (at_rover0_waypoint1) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective1_high_res) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint2_objective1_camera0_colour
  :parameters ()
  :precondition (and (at_rover0_waypoint2) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective1_colour) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint2_objective1_camera0_high_res
  :parameters ()
  :precondition (and (at_rover0_waypoint2) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective1_high_res) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint3_objective1_camera0_colour
  :parameters ()
  :precondition (and (at_rover0_waypoint3) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective1_colour) (not (calibrated_camera0_rover0))))
 (:action take_image_rover0_waypoint3_objective1_camera0_high_res
  :parameters ()
  :precondition (and (at_rover0_waypoint3) (calibrated_camera0_rover0))
  :effect (and (have_image_rover0_objective1_high_res) (not (calibrated_camera0_rover0))))
 (:action calibrate_rover0_camera0_objective1_waypoint0
  :parameters ()
  :precondition (and (at_rover0_waypoint0))
  :effect (and (calibrated_camera0_rover0)))
 (:action calibrate_rover0_camera0_objective1_waypoint1
  :parameters ()
  :precondition (and (at_rover0_waypoint1))
  :effect (and (calibrated_camera0_rover0)))
 (:action calibrate_rover0_camera0_objective1_waypoint2
  :parameters ()
  :precondition (and (at_rover0_waypoint2))
  :effect (and (calibrated_camera0_rover0)))
 (:action calibrate_rover0_camera0_objective1_waypoint3
  :parameters ()
  :precondition (and (at_rover0_waypoint3))
  :effect (and (calibrated_camera0_rover0)))
 (:action drop_rover0_rover0store
  :parameters ()
  :precondition (and (full_rover0store))
  :effect (and (empty_rover0store) (not (full_rover0store))))
 (:action sample_rock_rover0_rover0store_waypoint3
  :parameters ()
  :precondition (and (empty_rover0store) (at_rock_sample_waypoint3) (at_rover0_waypoint3))
  :effect (and (full_rover0store) (have_rock_analysis_rover0_waypoint3) (not (empty_rover0store)) (not (at_rock_sample_waypoint3))))
 (:action sample_rock_rover0_rover0store_waypoint2
  :parameters ()
  :precondition (and (empty_rover0store) (at_rock_sample_waypoint2) (at_rover0_waypoint2))
  :effect (and (full_rover0store) (have_rock_analysis_rover0_waypoint2) (not (empty_rover0store)) (not (at_rock_sample_waypoint2))))
 (:action sample_rock_rover0_rover0store_waypoint1
  :parameters ()
  :precondition (and (empty_rover0store) (at_rock_sample_waypoint1) (at_rover0_waypoint1))
  :effect (and (full_rover0store) (have_rock_analysis_rover0_waypoint1) (not (empty_rover0store)) (not (at_rock_sample_waypoint1))))
 (:action sample_soil_rover0_rover0store_waypoint3
  :parameters ()
  :precondition (and (empty_rover0store) (at_soil_sample_waypoint3) (at_rover0_waypoint3))
  :effect (and (full_rover0store) (have_soil_analysis_rover0_waypoint3) (not (empty_rover0store)) (not (at_soil_sample_waypoint3))))
 (:action sample_soil_rover0_rover0store_waypoint2
  :parameters ()
  :precondition (and (empty_rover0store) (at_soil_sample_waypoint2) (at_rover0_waypoint2))
  :effect (and (full_rover0store) (have_soil_analysis_rover0_waypoint2) (not (empty_rover0store)) (not (at_soil_sample_waypoint2))))
 (:action sample_soil_rover0_rover0store_waypoint0
  :parameters ()
  :precondition (and (empty_rover0store) (at_soil_sample_waypoint0) (at_rover0_waypoint0))
  :effect (and (full_rover0store) (have_soil_analysis_rover0_waypoint0) (not (empty_rover0store)) (not (at_soil_sample_waypoint0))))
 (:action navigate_rover0_waypoint2_waypoint1
  :parameters ()
  :precondition (and (at_rover0_waypoint2) (available_rover0))
  :effect (and (at_rover0_waypoint1) (not (at_rover0_waypoint2))))
 (:action navigate_rover0_waypoint1_waypoint2
  :parameters ()
  :precondition (and (at_rover0_waypoint1) (available_rover0))
  :effect (and (at_rover0_waypoint2) (not (at_rover0_waypoint1))))
 (:action navigate_rover0_waypoint3_waypoint0
  :parameters ()
  :precondition (and (at_rover0_waypoint3) (available_rover0))
  :effect (and (at_rover0_waypoint0) (not (at_rover0_waypoint3))))
 (:action navigate_rover0_waypoint3_waypoint1
  :parameters ()
  :precondition (and (at_rover0_waypoint3) (available_rover0))
  :effect (and (at_rover0_waypoint1) (not (at_rover0_waypoint3))))
)
