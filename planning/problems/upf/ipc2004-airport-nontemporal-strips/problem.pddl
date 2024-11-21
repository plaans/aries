(define (problem problem_x-problem)
 (:domain problem_x-domain)
 (:objects
   seg_rwte2_0_10 seg_rwtw2_0_10 - segment
   light heavy - airplanetype
 )
 (:init (at_segment airplane_cfbeg seg_rw_0_400) (blocked seg_rw_0_400 airplane_cfbeg) (blocked seg_rwe_0_50 airplane_cfbeg) (facing airplane_cfbeg south) (has_type airplane_cfbeg medium) (is_moving airplane_cfbeg) (not_blocked seg_pp_0_60 airplane_cfbeg) (not_blocked seg_ppdoor_0_40 airplane_cfbeg) (not_blocked seg_tww1_0_200 airplane_cfbeg) (not_blocked seg_twe1_0_200 airplane_cfbeg) (not_blocked seg_tww2_0_50 airplane_cfbeg) (not_blocked seg_tww3_0_50 airplane_cfbeg) (not_blocked seg_tww4_0_50 airplane_cfbeg) (not_blocked seg_rww_0_50 airplane_cfbeg) (not_blocked seg_rwtw1_0_10 airplane_cfbeg) (not_blocked seg_twe4_0_50 airplane_cfbeg) (not_blocked seg_rwte1_0_10 airplane_cfbeg) (not_blocked seg_twe3_0_50 airplane_cfbeg) (not_blocked seg_twe2_0_50 airplane_cfbeg) (not_blocked seg_rwte2_0_10 airplane_cfbeg) (not_blocked seg_rwtw2_0_10 airplane_cfbeg) (not_occupied seg_pp_0_60) (not_occupied seg_ppdoor_0_40) (not_occupied seg_tww1_0_200) (not_occupied seg_twe1_0_200) (not_occupied seg_tww2_0_50) (not_occupied seg_tww3_0_50) (not_occupied seg_tww4_0_50) (not_occupied seg_rww_0_50) (not_occupied seg_rwtw1_0_10) (not_occupied seg_rwe_0_50) (not_occupied seg_twe4_0_50) (not_occupied seg_rwte1_0_10) (not_occupied seg_twe3_0_50) (not_occupied seg_twe2_0_50) (not_occupied seg_rwte2_0_10) (not_occupied seg_rwtw2_0_10) (occupied seg_rw_0_400))
 (:goal (and (is_parked airplane_cfbeg seg_pp_0_60)))
)
