(define (domain grounded_pathways_01-domain)
 (:requirements :strips)
 (:predicates (foo) (chosen_sp1) (num_subs_l1) (chosen_raf1) (chosen_prbp2) (chosen_prb_e2f4p1_dp12) (chosen_pcaf) (chosen_p300) (chosen_p16) (chosen_p130_e2f5p1_dp12) (chosen_e2f13) (chosen_dmp1) (chosen_chk1) (chosen_cdk7) (chosen_cdk46p3_cycdp1) (chosen_cdk46p3_cycd) (chosen_cdc25c) (chosen_ap2) (available_sp1) (available_raf1) (available_prbp2) (available_prb_e2f4p1_dp12) (available_pcaf) (available_p300) (available_p16) (available_p130_e2f5p1_dp12) (available_e2f13) (available_dmp1) (available_chk1) (available_cdk7) (available_cdk46p3_cycdp1) (available_cdk46p3_cycd) (available_cdc25c) (available_ap2) (available_sp1_e2f13) (available_raf1_prb_e2f4p1_dp12) (available_raf1_p130_e2f5p1_dp12) (available_prbp2_ap2) (available_pcaf_p300) (available_p16_cdk7) (available_prbp1p2) (available_dmp1p1) (available_cdc25cp2) (goal1_) (num_subs_l2) (available_prbp1p2_ap2) (num_subs_l3) (not_chosen_ap2) (not_chosen_cdc25c) (not_chosen_cdk46p3_cycd) (not_chosen_cdk46p3_cycdp1) (not_chosen_cdk7) (not_chosen_chk1) (not_chosen_dmp1) (not_chosen_e2f13) (not_chosen_p130_e2f5p1_dp12) (not_chosen_p16) (not_chosen_p300) (not_chosen_pcaf) (not_chosen_prb_e2f4p1_dp12) (not_chosen_prbp2) (not_chosen_raf1) (not_chosen_sp1) (num_subs_l0))
 (:action choose_ap2_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_ap2))
  :effect (and (chosen_ap2) (num_subs_l3) (not (not_chosen_ap2)) (not (num_subs_l2))))
 (:action choose_cdc25c_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_cdc25c))
  :effect (and (chosen_cdc25c) (num_subs_l3) (not (not_chosen_cdc25c)) (not (num_subs_l2))))
 (:action choose_cdk46p3_cycd_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_cdk46p3_cycd))
  :effect (and (chosen_cdk46p3_cycd) (num_subs_l3) (not (not_chosen_cdk46p3_cycd)) (not (num_subs_l2))))
 (:action choose_cdk46p3_cycdp1_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_cdk46p3_cycdp1))
  :effect (and (chosen_cdk46p3_cycdp1) (num_subs_l3) (not (not_chosen_cdk46p3_cycdp1)) (not (num_subs_l2))))
 (:action choose_cdk7_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_cdk7))
  :effect (and (chosen_cdk7) (num_subs_l3) (not (not_chosen_cdk7)) (not (num_subs_l2))))
 (:action choose_chk1_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_chk1))
  :effect (and (chosen_chk1) (num_subs_l3) (not (not_chosen_chk1)) (not (num_subs_l2))))
 (:action choose_dmp1_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_dmp1))
  :effect (and (chosen_dmp1) (num_subs_l3) (not (not_chosen_dmp1)) (not (num_subs_l2))))
 (:action choose_e2f13_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_e2f13))
  :effect (and (chosen_e2f13) (num_subs_l3) (not (not_chosen_e2f13)) (not (num_subs_l2))))
 (:action choose_p130_e2f5p1_dp12_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_p130_e2f5p1_dp12))
  :effect (and (chosen_p130_e2f5p1_dp12) (num_subs_l3) (not (not_chosen_p130_e2f5p1_dp12)) (not (num_subs_l2))))
 (:action choose_p16_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_p16))
  :effect (and (chosen_p16) (num_subs_l3) (not (not_chosen_p16)) (not (num_subs_l2))))
 (:action choose_p300_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_p300))
  :effect (and (chosen_p300) (num_subs_l3) (not (not_chosen_p300)) (not (num_subs_l2))))
 (:action choose_pcaf_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_pcaf))
  :effect (and (chosen_pcaf) (num_subs_l3) (not (not_chosen_pcaf)) (not (num_subs_l2))))
 (:action choose_prb_e2f4p1_dp12_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_prb_e2f4p1_dp12))
  :effect (and (chosen_prb_e2f4p1_dp12) (num_subs_l3) (not (not_chosen_prb_e2f4p1_dp12)) (not (num_subs_l2))))
 (:action choose_prbp2_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_prbp2))
  :effect (and (chosen_prbp2) (num_subs_l3) (not (not_chosen_prbp2)) (not (num_subs_l2))))
 (:action choose_raf1_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_raf1))
  :effect (and (chosen_raf1) (num_subs_l3) (not (not_chosen_raf1)) (not (num_subs_l2))))
 (:action choose_sp1_l3_l2
  :parameters ()
  :precondition (and (num_subs_l2) (not_chosen_sp1))
  :effect (and (chosen_sp1) (num_subs_l3) (not (not_chosen_sp1)) (not (num_subs_l2))))
 (:action dummy_action_1_1
  :parameters ()
  :precondition (and (available_prbp1p2_ap2))
  :effect (and (goal1_)))
 (:action associate_prbp1p2_ap2_prbp1p2_ap2
  :parameters ()
  :precondition (and (available_ap2) (available_prbp1p2))
  :effect (and (available_prbp1p2_ap2) (not (available_prbp1p2)) (not (available_ap2))))
 (:action choose_ap2_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_ap2))
  :effect (and (chosen_ap2) (num_subs_l2) (not (not_chosen_ap2)) (not (num_subs_l1))))
 (:action choose_cdc25c_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_cdc25c))
  :effect (and (chosen_cdc25c) (num_subs_l2) (not (not_chosen_cdc25c)) (not (num_subs_l1))))
 (:action choose_cdk46p3_cycd_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_cdk46p3_cycd))
  :effect (and (chosen_cdk46p3_cycd) (num_subs_l2) (not (not_chosen_cdk46p3_cycd)) (not (num_subs_l1))))
 (:action choose_cdk46p3_cycdp1_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_cdk46p3_cycdp1))
  :effect (and (chosen_cdk46p3_cycdp1) (num_subs_l2) (not (not_chosen_cdk46p3_cycdp1)) (not (num_subs_l1))))
 (:action choose_cdk7_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_cdk7))
  :effect (and (chosen_cdk7) (num_subs_l2) (not (not_chosen_cdk7)) (not (num_subs_l1))))
 (:action choose_chk1_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_chk1))
  :effect (and (chosen_chk1) (num_subs_l2) (not (not_chosen_chk1)) (not (num_subs_l1))))
 (:action choose_dmp1_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_dmp1))
  :effect (and (chosen_dmp1) (num_subs_l2) (not (not_chosen_dmp1)) (not (num_subs_l1))))
 (:action choose_e2f13_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_e2f13))
  :effect (and (chosen_e2f13) (num_subs_l2) (not (not_chosen_e2f13)) (not (num_subs_l1))))
 (:action choose_p130_e2f5p1_dp12_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_p130_e2f5p1_dp12))
  :effect (and (chosen_p130_e2f5p1_dp12) (num_subs_l2) (not (not_chosen_p130_e2f5p1_dp12)) (not (num_subs_l1))))
 (:action choose_p16_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_p16))
  :effect (and (chosen_p16) (num_subs_l2) (not (not_chosen_p16)) (not (num_subs_l1))))
 (:action choose_p300_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_p300))
  :effect (and (chosen_p300) (num_subs_l2) (not (not_chosen_p300)) (not (num_subs_l1))))
 (:action choose_pcaf_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_pcaf))
  :effect (and (chosen_pcaf) (num_subs_l2) (not (not_chosen_pcaf)) (not (num_subs_l1))))
 (:action choose_prb_e2f4p1_dp12_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_prb_e2f4p1_dp12))
  :effect (and (chosen_prb_e2f4p1_dp12) (num_subs_l2) (not (not_chosen_prb_e2f4p1_dp12)) (not (num_subs_l1))))
 (:action choose_prbp2_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_prbp2))
  :effect (and (chosen_prbp2) (num_subs_l2) (not (not_chosen_prbp2)) (not (num_subs_l1))))
 (:action choose_raf1_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_raf1))
  :effect (and (chosen_raf1) (num_subs_l2) (not (not_chosen_raf1)) (not (num_subs_l1))))
 (:action choose_sp1_l2_l1
  :parameters ()
  :precondition (and (num_subs_l1) (not_chosen_sp1))
  :effect (and (chosen_sp1) (num_subs_l2) (not (not_chosen_sp1)) (not (num_subs_l1))))
 (:action dummy_action_1_2
  :parameters ()
  :precondition (and (available_pcaf_p300))
  :effect (and (goal1_)))
 (:action associate_with_catalyze_cdc25c_chk1_cdc25cp2
  :parameters ()
  :precondition (and (available_chk1) (available_cdc25c))
  :effect (and (available_cdc25cp2) (not (available_cdc25c))))
 (:action associate_with_catalyze_dmp1_cdk46p3_cycd_dmp1p1
  :parameters ()
  :precondition (and (available_cdk46p3_cycd) (available_dmp1))
  :effect (and (available_dmp1p1) (not (available_dmp1))))
 (:action associate_with_catalyze_dmp1_cdk46p3_cycdp1_dmp1p1
  :parameters ()
  :precondition (and (available_cdk46p3_cycdp1) (available_dmp1))
  :effect (and (available_dmp1p1) (not (available_dmp1))))
 (:action associate_with_catalyze_prbp2_cdk46p3_cycdp1_prbp1p2
  :parameters ()
  :precondition (and (available_cdk46p3_cycdp1) (available_prbp2))
  :effect (and (available_prbp1p2) (not (available_prbp2))))
 (:action associate_with_catalyze_prbp2_cdk46p3_cycd_prbp1p2
  :parameters ()
  :precondition (and (available_cdk46p3_cycd) (available_prbp2))
  :effect (and (available_prbp1p2) (not (available_prbp2))))
 (:action associate_p16_cdk7_p16_cdk7
  :parameters ()
  :precondition (and (available_cdk7) (available_p16))
  :effect (and (available_p16_cdk7) (not (available_p16)) (not (available_cdk7))))
 (:action associate_pcaf_p300_pcaf_p300
  :parameters ()
  :precondition (and (available_p300) (available_pcaf))
  :effect (and (available_pcaf_p300) (not (available_pcaf)) (not (available_p300))))
 (:action associate_prbp2_ap2_prbp2_ap2
  :parameters ()
  :precondition (and (available_ap2) (available_prbp2))
  :effect (and (available_prbp2_ap2) (not (available_prbp2)) (not (available_ap2))))
 (:action associate_raf1_p130_e2f5p1_dp12_raf1_p130_e2f5p1_dp12
  :parameters ()
  :precondition (and (available_p130_e2f5p1_dp12) (available_raf1))
  :effect (and (available_raf1_p130_e2f5p1_dp12) (not (available_raf1)) (not (available_p130_e2f5p1_dp12))))
 (:action associate_raf1_prb_e2f4p1_dp12_raf1_prb_e2f4p1_dp12
  :parameters ()
  :precondition (and (available_prb_e2f4p1_dp12) (available_raf1))
  :effect (and (available_raf1_prb_e2f4p1_dp12) (not (available_raf1)) (not (available_prb_e2f4p1_dp12))))
 (:action associate_sp1_e2f13_sp1_e2f13
  :parameters ()
  :precondition (and (available_e2f13) (available_sp1))
  :effect (and (available_sp1_e2f13) (not (available_sp1)) (not (available_e2f13))))
 (:action initialize_ap2
  :parameters ()
  :precondition (and (chosen_ap2))
  :effect (and (available_ap2)))
 (:action initialize_cdc25c
  :parameters ()
  :precondition (and (chosen_cdc25c))
  :effect (and (available_cdc25c)))
 (:action initialize_cdk46p3_cycd
  :parameters ()
  :precondition (and (chosen_cdk46p3_cycd))
  :effect (and (available_cdk46p3_cycd)))
 (:action initialize_cdk46p3_cycdp1
  :parameters ()
  :precondition (and (chosen_cdk46p3_cycdp1))
  :effect (and (available_cdk46p3_cycdp1)))
 (:action initialize_cdk7
  :parameters ()
  :precondition (and (chosen_cdk7))
  :effect (and (available_cdk7)))
 (:action initialize_chk1
  :parameters ()
  :precondition (and (chosen_chk1))
  :effect (and (available_chk1)))
 (:action initialize_dmp1
  :parameters ()
  :precondition (and (chosen_dmp1))
  :effect (and (available_dmp1)))
 (:action initialize_e2f13
  :parameters ()
  :precondition (and (chosen_e2f13))
  :effect (and (available_e2f13)))
 (:action initialize_p130_e2f5p1_dp12
  :parameters ()
  :precondition (and (chosen_p130_e2f5p1_dp12))
  :effect (and (available_p130_e2f5p1_dp12)))
 (:action initialize_p16
  :parameters ()
  :precondition (and (chosen_p16))
  :effect (and (available_p16)))
 (:action initialize_p300
  :parameters ()
  :precondition (and (chosen_p300))
  :effect (and (available_p300)))
 (:action initialize_pcaf
  :parameters ()
  :precondition (and (chosen_pcaf))
  :effect (and (available_pcaf)))
 (:action initialize_prb_e2f4p1_dp12
  :parameters ()
  :precondition (and (chosen_prb_e2f4p1_dp12))
  :effect (and (available_prb_e2f4p1_dp12)))
 (:action initialize_prbp2
  :parameters ()
  :precondition (and (chosen_prbp2))
  :effect (and (available_prbp2)))
 (:action initialize_raf1
  :parameters ()
  :precondition (and (chosen_raf1))
  :effect (and (available_raf1)))
 (:action initialize_sp1
  :parameters ()
  :precondition (and (chosen_sp1))
  :effect (and (available_sp1)))
 (:action choose_ap2_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_ap2))
  :effect (and (chosen_ap2) (num_subs_l1) (not (not_chosen_ap2)) (not (num_subs_l0))))
 (:action choose_cdc25c_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_cdc25c))
  :effect (and (chosen_cdc25c) (num_subs_l1) (not (not_chosen_cdc25c)) (not (num_subs_l0))))
 (:action choose_cdk46p3_cycd_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_cdk46p3_cycd))
  :effect (and (chosen_cdk46p3_cycd) (num_subs_l1) (not (not_chosen_cdk46p3_cycd)) (not (num_subs_l0))))
 (:action choose_cdk46p3_cycdp1_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_cdk46p3_cycdp1))
  :effect (and (chosen_cdk46p3_cycdp1) (num_subs_l1) (not (not_chosen_cdk46p3_cycdp1)) (not (num_subs_l0))))
 (:action choose_cdk7_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_cdk7))
  :effect (and (chosen_cdk7) (num_subs_l1) (not (not_chosen_cdk7)) (not (num_subs_l0))))
 (:action choose_chk1_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_chk1))
  :effect (and (chosen_chk1) (num_subs_l1) (not (not_chosen_chk1)) (not (num_subs_l0))))
 (:action choose_dmp1_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_dmp1))
  :effect (and (chosen_dmp1) (num_subs_l1) (not (not_chosen_dmp1)) (not (num_subs_l0))))
 (:action choose_e2f13_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_e2f13))
  :effect (and (chosen_e2f13) (num_subs_l1) (not (not_chosen_e2f13)) (not (num_subs_l0))))
 (:action choose_p130_e2f5p1_dp12_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_p130_e2f5p1_dp12))
  :effect (and (chosen_p130_e2f5p1_dp12) (num_subs_l1) (not (not_chosen_p130_e2f5p1_dp12)) (not (num_subs_l0))))
 (:action choose_p16_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_p16))
  :effect (and (chosen_p16) (num_subs_l1) (not (not_chosen_p16)) (not (num_subs_l0))))
 (:action choose_p300_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_p300))
  :effect (and (chosen_p300) (num_subs_l1) (not (not_chosen_p300)) (not (num_subs_l0))))
 (:action choose_pcaf_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_pcaf))
  :effect (and (chosen_pcaf) (num_subs_l1) (not (not_chosen_pcaf)) (not (num_subs_l0))))
 (:action choose_prb_e2f4p1_dp12_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_prb_e2f4p1_dp12))
  :effect (and (chosen_prb_e2f4p1_dp12) (num_subs_l1) (not (not_chosen_prb_e2f4p1_dp12)) (not (num_subs_l0))))
 (:action choose_prbp2_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_prbp2))
  :effect (and (chosen_prbp2) (num_subs_l1) (not (not_chosen_prbp2)) (not (num_subs_l0))))
 (:action choose_raf1_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_raf1))
  :effect (and (chosen_raf1) (num_subs_l1) (not (not_chosen_raf1)) (not (num_subs_l0))))
 (:action choose_sp1_l1_l0
  :parameters ()
  :precondition (and (num_subs_l0) (not_chosen_sp1))
  :effect (and (chosen_sp1) (num_subs_l1) (not (not_chosen_sp1)) (not (num_subs_l0))))
)
