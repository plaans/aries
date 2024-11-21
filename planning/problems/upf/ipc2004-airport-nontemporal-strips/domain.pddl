(define (domain problem_x-domain)
 (:requirements :strips :typing)
 (:types airplane segment direction airplanetype)
 (:constants
   medium - airplanetype
   south north - direction
   seg_pp_0_60 seg_tww4_0_50 seg_rw_0_400 seg_twe1_0_200 seg_tww1_0_200 seg_twe3_0_50 seg_tww3_0_50 seg_rwte1_0_10 seg_tww2_0_50 seg_twe2_0_50 seg_rwe_0_50 seg_rww_0_50 seg_twe4_0_50 seg_ppdoor_0_40 seg_rwtw1_0_10 - segment
   airplane_cfbeg - airplane
 )
 (:predicates (has_type ?a - airplane ?t - airplanetype) (at_segment ?a - airplane ?s - segment) (facing ?a - airplane ?d - direction) (occupied ?s - segment) (not_occupied ?s - segment) (blocked ?s - segment ?a - airplane) (not_blocked ?s - segment ?a - airplane) (is_start_runway ?s - segment ?d - direction) (airborne ?a - airplane ?s - segment) (is_moving ?a - airplane) (is_pushing ?a - airplane) (is_parked ?a - airplane ?s - segment))
 (:action move_seg_pp_0_60_seg_ppdoor_0_40_north_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a north) (at_segment ?a seg_pp_0_60) (not_occupied seg_ppdoor_0_40) (not_blocked seg_ppdoor_0_40 airplane_cfbeg))
  :effect (and (not (occupied seg_pp_0_60)) (not_occupied seg_pp_0_60) (not (at_segment ?a seg_pp_0_60)) (occupied seg_ppdoor_0_40) (not (not_occupied seg_ppdoor_0_40)) (blocked seg_ppdoor_0_40 ?a) (not (not_blocked seg_ppdoor_0_40 ?a)) (at_segment ?a seg_ppdoor_0_40)))
 (:action move_seg_ppdoor_0_40_seg_tww1_0_200_north_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a north) (at_segment ?a seg_ppdoor_0_40) (not_occupied seg_tww1_0_200) (not_blocked seg_tww1_0_200 airplane_cfbeg) (not_occupied seg_twe1_0_200))
  :effect (and (not (occupied seg_ppdoor_0_40)) (not_occupied seg_ppdoor_0_40) (not (at_segment ?a seg_ppdoor_0_40)) (occupied seg_tww1_0_200) (not (not_occupied seg_tww1_0_200)) (blocked seg_tww1_0_200 ?a) (not (not_blocked seg_tww1_0_200 ?a)) (at_segment ?a seg_tww1_0_200) (not (facing ?a north)) (facing ?a south) (not (blocked seg_pp_0_60 ?a)) (not_blocked seg_pp_0_60 ?a) (blocked seg_twe1_0_200 ?a) (not (not_blocked seg_twe1_0_200 ?a))))
 (:action move_seg_tww1_0_200_seg_twe1_0_200_north_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a north) (at_segment ?a seg_tww1_0_200) (not_occupied seg_twe1_0_200) (not_blocked seg_twe1_0_200 airplane_cfbeg) (not_occupied seg_ppdoor_0_40))
  :effect (and (not (occupied seg_tww1_0_200)) (not_occupied seg_tww1_0_200) (not (at_segment ?a seg_tww1_0_200)) (occupied seg_twe1_0_200) (not (not_occupied seg_twe1_0_200)) (blocked seg_twe1_0_200 ?a) (not (not_blocked seg_twe1_0_200 ?a)) (at_segment ?a seg_twe1_0_200) (not (facing ?a north)) (facing ?a south) (not (blocked seg_tww2_0_50 ?a)) (not_blocked seg_tww2_0_50 ?a) (blocked seg_ppdoor_0_40 ?a) (not (not_blocked seg_ppdoor_0_40 ?a))))
 (:action move_seg_twe1_0_200_seg_twe2_0_50_south_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a south) (at_segment ?a seg_twe1_0_200) (not_occupied seg_twe2_0_50) (not_blocked seg_twe2_0_50 airplane_cfbeg))
  :effect (and (not (occupied seg_twe1_0_200)) (not_occupied seg_twe1_0_200) (not (at_segment ?a seg_twe1_0_200)) (occupied seg_twe2_0_50) (not (not_occupied seg_twe2_0_50)) (blocked seg_twe2_0_50 ?a) (not (not_blocked seg_twe2_0_50 ?a)) (at_segment ?a seg_twe2_0_50) (not (blocked seg_ppdoor_0_40 ?a)) (not_blocked seg_ppdoor_0_40 ?a) (not (blocked seg_tww1_0_200 ?a)) (not_blocked seg_tww1_0_200 ?a)))
 (:action move_seg_twe2_0_50_seg_twe3_0_50_south_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a south) (at_segment ?a seg_twe2_0_50) (not_occupied seg_twe3_0_50) (not_blocked seg_twe3_0_50 airplane_cfbeg))
  :effect (and (not (occupied seg_twe2_0_50)) (not_occupied seg_twe2_0_50) (not (at_segment ?a seg_twe2_0_50)) (occupied seg_twe3_0_50) (not (not_occupied seg_twe3_0_50)) (blocked seg_twe3_0_50 ?a) (not (not_blocked seg_twe3_0_50 ?a)) (at_segment ?a seg_twe3_0_50) (not (blocked seg_twe1_0_200 ?a)) (not_blocked seg_twe1_0_200 ?a)))
 (:action move_seg_twe3_0_50_seg_twe4_0_50_south_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a south) (at_segment ?a seg_twe3_0_50) (not_occupied seg_twe4_0_50) (not_blocked seg_twe4_0_50 airplane_cfbeg))
  :effect (and (not (occupied seg_twe3_0_50)) (not_occupied seg_twe3_0_50) (not (at_segment ?a seg_twe3_0_50)) (occupied seg_twe4_0_50) (not (not_occupied seg_twe4_0_50)) (blocked seg_twe4_0_50 ?a) (not (not_blocked seg_twe4_0_50 ?a)) (at_segment ?a seg_twe4_0_50) (not (blocked seg_twe2_0_50 ?a)) (not_blocked seg_twe2_0_50 ?a)))
 (:action move_seg_twe4_0_50_seg_rwe_0_50_south_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a south) (at_segment ?a seg_twe4_0_50) (not_occupied seg_rwe_0_50) (not_blocked seg_rwe_0_50 airplane_cfbeg))
  :effect (and (not (occupied seg_twe4_0_50)) (not_occupied seg_twe4_0_50) (not (at_segment ?a seg_twe4_0_50)) (occupied seg_rwe_0_50) (not (not_occupied seg_rwe_0_50)) (blocked seg_rwe_0_50 ?a) (not (not_blocked seg_rwe_0_50 ?a)) (at_segment ?a seg_rwe_0_50) (not (blocked seg_twe3_0_50 ?a)) (not_blocked seg_twe3_0_50 ?a)))
 (:action move_seg_rwe_0_50_seg_rw_0_400_south_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a south) (at_segment ?a seg_rwe_0_50) (not_occupied seg_rw_0_400) (not_blocked seg_rw_0_400 airplane_cfbeg))
  :effect (and (not (occupied seg_rwe_0_50)) (not_occupied seg_rwe_0_50) (not (at_segment ?a seg_rwe_0_50)) (occupied seg_rw_0_400) (not (not_occupied seg_rw_0_400)) (blocked seg_rw_0_400 ?a) (not (not_blocked seg_rw_0_400 ?a)) (at_segment ?a seg_rw_0_400) (not (blocked seg_twe4_0_50 ?a)) (not_blocked seg_twe4_0_50 ?a)))
 (:action move_seg_rw_0_400_seg_rww_0_50_south_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a south) (at_segment ?a seg_rw_0_400) (not_occupied seg_rww_0_50) (not_blocked seg_rww_0_50 airplane_cfbeg) (not_occupied seg_rwe_0_50))
  :effect (and (not (occupied seg_rw_0_400)) (not_occupied seg_rw_0_400) (not (at_segment ?a seg_rw_0_400)) (occupied seg_rww_0_50) (not (not_occupied seg_rww_0_50)) (blocked seg_rww_0_50 ?a) (not (not_blocked seg_rww_0_50 ?a)) (at_segment ?a seg_rww_0_50)))
 (:action move_seg_rww_0_50_seg_tww4_0_50_south_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a south) (at_segment ?a seg_rww_0_50) (not_occupied seg_tww4_0_50) (not_blocked seg_tww4_0_50 airplane_cfbeg) (not_occupied seg_rwtw1_0_10))
  :effect (and (not (occupied seg_rww_0_50)) (not_occupied seg_rww_0_50) (not (at_segment ?a seg_rww_0_50)) (occupied seg_tww4_0_50) (not (not_occupied seg_tww4_0_50)) (blocked seg_tww4_0_50 ?a) (not (not_blocked seg_tww4_0_50 ?a)) (at_segment ?a seg_tww4_0_50) (not (facing ?a south)) (facing ?a north) (not (blocked seg_rwe_0_50 ?a)) (not_blocked seg_rwe_0_50 ?a) (not (blocked seg_rw_0_400 ?a)) (not_blocked seg_rw_0_400 ?a) (blocked seg_rwtw1_0_10 ?a) (not (not_blocked seg_rwtw1_0_10 ?a))))
 (:action move_seg_tww4_0_50_seg_tww3_0_50_north_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a north) (at_segment ?a seg_tww4_0_50) (not_occupied seg_tww3_0_50) (not_blocked seg_tww3_0_50 airplane_cfbeg))
  :effect (and (not (occupied seg_tww4_0_50)) (not_occupied seg_tww4_0_50) (not (at_segment ?a seg_tww4_0_50)) (occupied seg_tww3_0_50) (not (not_occupied seg_tww3_0_50)) (blocked seg_tww3_0_50 ?a) (not (not_blocked seg_tww3_0_50 ?a)) (at_segment ?a seg_tww3_0_50) (not (blocked seg_rww_0_50 ?a)) (not_blocked seg_rww_0_50 ?a) (not (blocked seg_rwtw1_0_10 ?a)) (not_blocked seg_rwtw1_0_10 ?a)))
 (:action move_seg_tww3_0_50_seg_tww2_0_50_north_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a north) (at_segment ?a seg_tww3_0_50) (not_occupied seg_tww2_0_50) (not_blocked seg_tww2_0_50 airplane_cfbeg))
  :effect (and (not (occupied seg_tww3_0_50)) (not_occupied seg_tww3_0_50) (not (at_segment ?a seg_tww3_0_50)) (occupied seg_tww2_0_50) (not (not_occupied seg_tww2_0_50)) (blocked seg_tww2_0_50 ?a) (not (not_blocked seg_tww2_0_50 ?a)) (at_segment ?a seg_tww2_0_50) (not (blocked seg_tww4_0_50 ?a)) (not_blocked seg_tww4_0_50 ?a)))
 (:action move_seg_tww2_0_50_seg_tww1_0_200_north_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a north) (at_segment ?a seg_tww2_0_50) (not_occupied seg_tww1_0_200) (not_blocked seg_tww1_0_200 airplane_cfbeg))
  :effect (and (not (occupied seg_tww2_0_50)) (not_occupied seg_tww2_0_50) (not (at_segment ?a seg_tww2_0_50)) (occupied seg_tww1_0_200) (not (not_occupied seg_tww1_0_200)) (blocked seg_tww1_0_200 ?a) (not (not_blocked seg_tww1_0_200 ?a)) (at_segment ?a seg_tww1_0_200) (not (blocked seg_tww3_0_50 ?a)) (not_blocked seg_tww3_0_50 ?a)))
 (:action move_seg_tww1_0_200_seg_ppdoor_0_40_north_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a north) (at_segment ?a seg_tww1_0_200) (not_occupied seg_ppdoor_0_40) (not_blocked seg_ppdoor_0_40 airplane_cfbeg) (not_occupied seg_twe1_0_200))
  :effect (and (not (occupied seg_tww1_0_200)) (not_occupied seg_tww1_0_200) (not (at_segment ?a seg_tww1_0_200)) (occupied seg_ppdoor_0_40) (not (not_occupied seg_ppdoor_0_40)) (blocked seg_ppdoor_0_40 ?a) (not (not_blocked seg_ppdoor_0_40 ?a)) (at_segment ?a seg_ppdoor_0_40) (not (facing ?a north)) (facing ?a south) (not (blocked seg_tww2_0_50 ?a)) (not_blocked seg_tww2_0_50 ?a) (blocked seg_twe1_0_200 ?a) (not (not_blocked seg_twe1_0_200 ?a))))
 (:action move_seg_ppdoor_0_40_seg_pp_0_60_south_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (has_type ?a medium) (is_moving ?a) (facing ?a south) (at_segment ?a seg_ppdoor_0_40) (not_occupied seg_pp_0_60) (not_blocked seg_pp_0_60 airplane_cfbeg))
  :effect (and (not (occupied seg_ppdoor_0_40)) (not_occupied seg_ppdoor_0_40) (not (at_segment ?a seg_ppdoor_0_40)) (occupied seg_pp_0_60) (not (not_occupied seg_pp_0_60)) (blocked seg_pp_0_60 ?a) (not (not_blocked seg_pp_0_60 ?a)) (at_segment ?a seg_pp_0_60) (not (blocked seg_tww1_0_200 ?a)) (not_blocked seg_tww1_0_200 ?a) (not (blocked seg_twe1_0_200 ?a)) (not_blocked seg_twe1_0_200 ?a)))
 (:action takeoff_seg_rww_0_50_north
  :parameters ( ?a - airplane)
  :precondition (and (at_segment ?a seg_rww_0_50) (facing ?a north) (is_moving ?a))
  :effect (and (not (occupied seg_rww_0_50)) (not_occupied seg_rww_0_50) (not (blocked seg_rww_0_50 ?a)) (not_blocked seg_rww_0_50 ?a) (not (at_segment ?a seg_rww_0_50)) (airborne ?a seg_rww_0_50) (not (is_moving ?a)) (not (blocked seg_tww4_0_50 ?a)) (not_blocked seg_tww4_0_50 ?a)))
 (:action takeoff_seg_rwe_0_50_south
  :parameters ( ?a - airplane)
  :precondition (and (at_segment ?a seg_rwe_0_50) (facing ?a south) (is_moving ?a))
  :effect (and (not (occupied seg_rwe_0_50)) (not_occupied seg_rwe_0_50) (not (blocked seg_rwe_0_50 ?a)) (not_blocked seg_rwe_0_50 ?a) (not (at_segment ?a seg_rwe_0_50)) (airborne ?a seg_rwe_0_50) (not (is_moving ?a)) (not (blocked seg_twe4_0_50 ?a)) (not_blocked seg_twe4_0_50 ?a)))
 (:action park_seg_pp_0_60_north
  :parameters ( ?a - airplane)
  :precondition (and (at_segment ?a seg_pp_0_60) (facing ?a north) (is_moving ?a))
  :effect (and (is_parked ?a seg_pp_0_60) (not (is_moving ?a))))
 (:action park_seg_pp_0_60_south
  :parameters ( ?a - airplane)
  :precondition (and (at_segment ?a seg_pp_0_60) (facing ?a south) (is_moving ?a))
  :effect (and (is_parked ?a seg_pp_0_60) (not (is_moving ?a)) (not (blocked seg_ppdoor_0_40 ?a)) (not_blocked seg_ppdoor_0_40 ?a)))
 (:action startup_seg_pp_0_60_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_pp_0_60))
  :effect (and (not (is_pushing ?a)) (is_moving ?a)))
 (:action startup_seg_pp_0_60_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_pp_0_60) (not_occupied seg_ppdoor_0_40))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_ppdoor_0_40 ?a) (not (not_blocked seg_ppdoor_0_40 ?a))))
 (:action startup_seg_ppdoor_0_40_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_ppdoor_0_40) (not_occupied seg_pp_0_60))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_pp_0_60 ?a) (not (not_blocked seg_pp_0_60 ?a))))
 (:action startup_seg_ppdoor_0_40_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_ppdoor_0_40) (not_occupied seg_tww1_0_200) (not_occupied seg_twe1_0_200))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_tww1_0_200 ?a) (not (not_blocked seg_tww1_0_200 ?a)) (blocked seg_twe1_0_200 ?a) (not (not_blocked seg_twe1_0_200 ?a))))
 (:action startup_seg_tww1_0_200_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_tww1_0_200) (not_occupied seg_tww2_0_50))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_tww2_0_50 ?a) (not (not_blocked seg_tww2_0_50 ?a))))
 (:action startup_seg_tww1_0_200_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_tww1_0_200) (not_occupied seg_ppdoor_0_40) (not_occupied seg_twe1_0_200))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_ppdoor_0_40 ?a) (not (not_blocked seg_ppdoor_0_40 ?a)) (blocked seg_twe1_0_200 ?a) (not (not_blocked seg_twe1_0_200 ?a))))
 (:action startup_seg_twe1_0_200_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_twe1_0_200) (not_occupied seg_twe2_0_50))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_twe2_0_50 ?a) (not (not_blocked seg_twe2_0_50 ?a))))
 (:action startup_seg_twe1_0_200_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_twe1_0_200) (not_occupied seg_ppdoor_0_40) (not_occupied seg_tww1_0_200))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_ppdoor_0_40 ?a) (not (not_blocked seg_ppdoor_0_40 ?a)) (blocked seg_tww1_0_200 ?a) (not (not_blocked seg_tww1_0_200 ?a))))
 (:action startup_seg_tww2_0_50_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_tww2_0_50) (not_occupied seg_tww3_0_50))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_tww3_0_50 ?a) (not (not_blocked seg_tww3_0_50 ?a))))
 (:action startup_seg_tww2_0_50_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_tww2_0_50) (not_occupied seg_tww1_0_200))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_tww1_0_200 ?a) (not (not_blocked seg_tww1_0_200 ?a))))
 (:action startup_seg_tww3_0_50_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_tww3_0_50) (not_occupied seg_tww4_0_50))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_tww4_0_50 ?a) (not (not_blocked seg_tww4_0_50 ?a))))
 (:action startup_seg_tww3_0_50_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_tww3_0_50) (not_occupied seg_tww2_0_50))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_tww2_0_50 ?a) (not (not_blocked seg_tww2_0_50 ?a))))
 (:action startup_seg_tww4_0_50_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_tww4_0_50) (not_occupied seg_rww_0_50) (not_occupied seg_rwtw1_0_10))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_rww_0_50 ?a) (not (not_blocked seg_rww_0_50 ?a)) (blocked seg_rwtw1_0_10 ?a) (not (not_blocked seg_rwtw1_0_10 ?a))))
 (:action startup_seg_tww4_0_50_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_tww4_0_50) (not_occupied seg_tww3_0_50))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_tww3_0_50 ?a) (not (not_blocked seg_tww3_0_50 ?a))))
 (:action startup_seg_twe4_0_50_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_twe4_0_50) (not_occupied seg_rwte1_0_10) (not_occupied seg_rwe_0_50))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_rwte1_0_10 ?a) (not (not_blocked seg_rwte1_0_10 ?a)) (blocked seg_rwe_0_50 ?a) (not (not_blocked seg_rwe_0_50 ?a))))
 (:action startup_seg_twe4_0_50_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_twe4_0_50) (not_occupied seg_twe3_0_50))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_twe3_0_50 ?a) (not (not_blocked seg_twe3_0_50 ?a))))
 (:action startup_seg_twe3_0_50_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_twe3_0_50) (not_occupied seg_twe4_0_50))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_twe4_0_50 ?a) (not (not_blocked seg_twe4_0_50 ?a))))
 (:action startup_seg_twe3_0_50_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_twe3_0_50) (not_occupied seg_twe2_0_50))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_twe2_0_50 ?a) (not (not_blocked seg_twe2_0_50 ?a))))
 (:action startup_seg_twe2_0_50_north_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_twe2_0_50) (not_occupied seg_twe3_0_50))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_twe3_0_50 ?a) (not (not_blocked seg_twe3_0_50 ?a))))
 (:action startup_seg_twe2_0_50_south_medium
  :parameters ( ?a - airplane)
  :precondition (and (is_pushing ?a) (at_segment ?a seg_twe2_0_50) (not_occupied seg_twe1_0_200))
  :effect (and (not (is_pushing ?a)) (is_moving ?a) (blocked seg_twe1_0_200 ?a) (not (not_blocked seg_twe1_0_200 ?a))))
)
