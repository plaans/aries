(define (domain grounded_strips_p01_net1_b6_g2_rt0_instance-domain)
 (:requirements :strips)
 (:predicates (first_b0_s13) (last_b0_s13) (not_occupied_ta1_1_oc1b) (on_b5_a3) (occupied_ta3_1_oca1) (first_b2_s13) (last_b2_s13) (not_occupied_ta1_1_gasoleo) (on_b2_a3) (occupied_ta3_1_gasoleo) (first_b3_s13) (last_b3_s13) (not_occupied_ta1_1_rat_a) (first_b0_s12) (last_b0_s12) (on_b4_a2) (occupied_ta2_1_lco) (first_b3_s12) (last_b3_s12) (first_b2_s12) (last_b2_s12) (last_b1_s13) (first_b1_s13) (not_occupied_ta3_1_lco) (on_b5_a1) (occupied_ta1_1_oca1) (on_b1_a1) (occupied_ta1_1_lco) (on_b4_a1) (on_b0_a3) (occupied_ta3_1_oc1b) (first_b4_s13) (last_b4_s13) (on_b3_a3) (occupied_ta3_1_rat_a) (on_b4_a3) (on_b0_a2) (occupied_ta2_1_oc1b) (first_b5_s12) (last_b5_s12) (first_b1_s12) (last_b1_s12) (on_b5_a2) (occupied_ta2_1_oca1) (on_b3_a2) (occupied_ta2_1_rat_a) (on_b2_a2) (occupied_ta2_1_gasoleo) (on_b1_a2) (not_occupied_ta1_1_lco) (last_b4_s12) (first_b4_s12) (occupied_ta3_1_lco) (on_b1_a3) (not_occupied_ta1_1_oca1) (last_b5_s13) (first_b5_s13) (not_occupied_ta2_1_lco) (on_b2_a1) (occupied_ta1_1_gasoleo) (on_b3_a1) (occupied_ta1_1_rat_a) (on_b0_a1) (occupied_ta1_1_oc1b) (not_occupied_ta2_1_gasoleo) (not_occupied_ta2_1_rat_a) (not_occupied_ta2_1_oca1) (not_occupied_ta2_1_oc1b) (not_occupied_ta3_1_gasoleo) (not_occupied_ta3_1_rat_a) (not_occupied_ta3_1_oca1) (not_occupied_ta3_1_oc1b))
 (:action pop_unitarypipe_s12_b1_a1_a2_b1_lco_lco_ta1_1_lco_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_lco) (on_b1_a2) (first_b1_s12))
  :effect (and (last_b1_s12) (first_b1_s12) (not_occupied_ta2_1_lco) (on_b1_a1) (occupied_ta1_1_lco) (not (on_b1_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b4_a1_a2_b1_lco_lco_ta1_1_lco_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_lco) (on_b4_a2) (first_b1_s12))
  :effect (and (last_b4_s12) (first_b4_s12) (not_occupied_ta2_1_lco) (on_b1_a1) (occupied_ta1_1_lco) (not (last_b1_s12)) (not (first_b1_s12)) (not (on_b4_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b1_a1_a2_b4_lco_lco_ta1_1_lco_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_lco) (on_b1_a2) (first_b4_s12))
  :effect (and (last_b1_s12) (first_b1_s12) (not_occupied_ta2_1_lco) (on_b4_a1) (occupied_ta1_1_lco) (not (last_b4_s12)) (not (first_b4_s12)) (not (on_b1_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b2_a1_a2_b1_gasoleo_lco_ta1_1_lco_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_gasoleo) (on_b2_a2) (first_b1_s12))
  :effect (and (last_b2_s12) (first_b2_s12) (not_occupied_ta2_1_gasoleo) (on_b1_a1) (occupied_ta1_1_lco) (not (last_b1_s12)) (not (first_b1_s12)) (not (on_b2_a2)) (not (occupied_ta2_1_gasoleo)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b2_a1_a2_b4_gasoleo_lco_ta1_1_lco_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_gasoleo) (on_b2_a2) (first_b4_s12))
  :effect (and (last_b2_s12) (first_b2_s12) (not_occupied_ta2_1_gasoleo) (on_b4_a1) (occupied_ta1_1_lco) (not (last_b4_s12)) (not (first_b4_s12)) (not (on_b2_a2)) (not (occupied_ta2_1_gasoleo)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b3_a1_a2_b1_rat_a_lco_ta1_1_lco_ta2_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_rat_a) (on_b3_a2) (first_b1_s12))
  :effect (and (last_b3_s12) (first_b3_s12) (not_occupied_ta2_1_rat_a) (on_b1_a1) (occupied_ta1_1_lco) (not (last_b1_s12)) (not (first_b1_s12)) (not (on_b3_a2)) (not (occupied_ta2_1_rat_a)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b3_a1_a2_b4_rat_a_lco_ta1_1_lco_ta2_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_rat_a) (on_b3_a2) (first_b4_s12))
  :effect (and (last_b3_s12) (first_b3_s12) (not_occupied_ta2_1_rat_a) (on_b4_a1) (occupied_ta1_1_lco) (not (last_b4_s12)) (not (first_b4_s12)) (not (on_b3_a2)) (not (occupied_ta2_1_rat_a)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b5_a1_a2_b1_oca1_lco_ta1_1_lco_ta2_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_oca1) (on_b5_a2) (first_b1_s12))
  :effect (and (last_b5_s12) (first_b5_s12) (not_occupied_ta2_1_oca1) (on_b1_a1) (occupied_ta1_1_lco) (not (last_b1_s12)) (not (first_b1_s12)) (not (on_b5_a2)) (not (occupied_ta2_1_oca1)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b5_a1_a2_b4_oca1_lco_ta1_1_lco_ta2_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_oca1) (on_b5_a2) (first_b4_s12))
  :effect (and (last_b5_s12) (first_b5_s12) (not_occupied_ta2_1_oca1) (on_b4_a1) (occupied_ta1_1_lco) (not (last_b4_s12)) (not (first_b4_s12)) (not (on_b5_a2)) (not (occupied_ta2_1_oca1)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b0_a1_a2_b1_oc1b_lco_ta1_1_lco_ta2_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_oc1b) (on_b0_a2) (first_b1_s12))
  :effect (and (last_b0_s12) (first_b0_s12) (not_occupied_ta2_1_oc1b) (on_b1_a1) (occupied_ta1_1_lco) (not (last_b1_s12)) (not (first_b1_s12)) (not (on_b0_a2)) (not (occupied_ta2_1_oc1b)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b0_a1_a2_b4_oc1b_lco_ta1_1_lco_ta2_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_oc1b) (on_b0_a2) (first_b4_s12))
  :effect (and (last_b0_s12) (first_b0_s12) (not_occupied_ta2_1_oc1b) (on_b4_a1) (occupied_ta1_1_lco) (not (last_b4_s12)) (not (first_b4_s12)) (not (on_b0_a2)) (not (occupied_ta2_1_oc1b)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b4_a1_a3_b1_lco_lco_ta1_1_lco_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_lco) (on_b4_a3) (first_b1_s13))
  :effect (and (last_b4_s13) (first_b4_s13) (not_occupied_ta3_1_lco) (on_b1_a1) (occupied_ta1_1_lco) (not (last_b1_s13)) (not (first_b1_s13)) (not (on_b4_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b1_a1_a3_b4_lco_lco_ta1_1_lco_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_lco) (on_b1_a3) (first_b4_s13))
  :effect (and (last_b1_s13) (first_b1_s13) (not_occupied_ta3_1_lco) (on_b4_a1) (occupied_ta1_1_lco) (not (last_b4_s13)) (not (first_b4_s13)) (not (on_b1_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b4_a1_a3_b4_lco_lco_ta1_1_lco_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_lco) (on_b4_a3) (first_b4_s13))
  :effect (and (last_b4_s13) (first_b4_s13) (not_occupied_ta3_1_lco) (on_b4_a1) (occupied_ta1_1_lco) (not (on_b4_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b2_a1_a3_b4_gasoleo_lco_ta1_1_lco_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_gasoleo) (on_b2_a3) (first_b4_s13))
  :effect (and (last_b2_s13) (first_b2_s13) (not_occupied_ta3_1_gasoleo) (on_b4_a1) (occupied_ta1_1_lco) (not (last_b4_s13)) (not (first_b4_s13)) (not (on_b2_a3)) (not (occupied_ta3_1_gasoleo)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b3_a1_a3_b1_rat_a_lco_ta1_1_lco_ta3_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_rat_a) (on_b3_a3) (first_b1_s13))
  :effect (and (last_b3_s13) (first_b3_s13) (not_occupied_ta3_1_rat_a) (on_b1_a1) (occupied_ta1_1_lco) (not (last_b1_s13)) (not (first_b1_s13)) (not (on_b3_a3)) (not (occupied_ta3_1_rat_a)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b3_a1_a3_b4_rat_a_lco_ta1_1_lco_ta3_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_rat_a) (on_b3_a3) (first_b4_s13))
  :effect (and (last_b3_s13) (first_b3_s13) (not_occupied_ta3_1_rat_a) (on_b4_a1) (occupied_ta1_1_lco) (not (last_b4_s13)) (not (first_b4_s13)) (not (on_b3_a3)) (not (occupied_ta3_1_rat_a)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b5_a1_a3_b4_oca1_lco_ta1_1_lco_ta3_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_oca1) (on_b5_a3) (first_b4_s13))
  :effect (and (last_b5_s13) (first_b5_s13) (not_occupied_ta3_1_oca1) (on_b4_a1) (occupied_ta1_1_lco) (not (last_b4_s13)) (not (first_b4_s13)) (not (on_b5_a3)) (not (occupied_ta3_1_oca1)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b0_a1_a3_b1_oc1b_lco_ta1_1_lco_ta3_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_oc1b) (on_b0_a3) (first_b1_s13))
  :effect (and (last_b0_s13) (first_b0_s13) (not_occupied_ta3_1_oc1b) (on_b1_a1) (occupied_ta1_1_lco) (not (last_b1_s13)) (not (first_b1_s13)) (not (on_b0_a3)) (not (occupied_ta3_1_oc1b)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b0_a1_a3_b4_oc1b_lco_ta1_1_lco_ta3_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_oc1b) (on_b0_a3) (first_b4_s13))
  :effect (and (last_b0_s13) (first_b0_s13) (not_occupied_ta3_1_oc1b) (on_b4_a1) (occupied_ta1_1_lco) (not (last_b4_s13)) (not (first_b4_s13)) (not (on_b0_a3)) (not (occupied_ta3_1_oc1b)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b1_a1_a2_b2_lco_gasoleo_ta1_1_gasoleo_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta2_1_lco) (on_b1_a2) (first_b2_s12))
  :effect (and (last_b1_s12) (first_b1_s12) (not_occupied_ta2_1_lco) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (last_b2_s12)) (not (first_b2_s12)) (not (on_b1_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s12_b2_a1_a2_b2_gasoleo_gasoleo_ta1_1_gasoleo_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta2_1_gasoleo) (on_b2_a2) (first_b2_s12))
  :effect (and (last_b2_s12) (first_b2_s12) (not_occupied_ta2_1_gasoleo) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (on_b2_a2)) (not (occupied_ta2_1_gasoleo)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s12_b3_a1_a2_b2_rat_a_gasoleo_ta1_1_gasoleo_ta2_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta2_1_rat_a) (on_b3_a2) (first_b2_s12))
  :effect (and (last_b3_s12) (first_b3_s12) (not_occupied_ta2_1_rat_a) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (last_b2_s12)) (not (first_b2_s12)) (not (on_b3_a2)) (not (occupied_ta2_1_rat_a)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s12_b5_a1_a2_b2_oca1_gasoleo_ta1_1_gasoleo_ta2_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta2_1_oca1) (on_b5_a2) (first_b2_s12))
  :effect (and (last_b5_s12) (first_b5_s12) (not_occupied_ta2_1_oca1) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (last_b2_s12)) (not (first_b2_s12)) (not (on_b5_a2)) (not (occupied_ta2_1_oca1)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s12_b0_a1_a2_b2_oc1b_gasoleo_ta1_1_gasoleo_ta2_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta2_1_oc1b) (on_b0_a2) (first_b2_s12))
  :effect (and (last_b0_s12) (first_b0_s12) (not_occupied_ta2_1_oc1b) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (last_b2_s12)) (not (first_b2_s12)) (not (on_b0_a2)) (not (occupied_ta2_1_oc1b)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s13_b4_a1_a3_b2_lco_gasoleo_ta1_1_gasoleo_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta3_1_lco) (on_b4_a3) (first_b2_s13))
  :effect (and (last_b4_s13) (first_b4_s13) (not_occupied_ta3_1_lco) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (last_b2_s13)) (not (first_b2_s13)) (not (on_b4_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s13_b3_a1_a3_b2_rat_a_gasoleo_ta1_1_gasoleo_ta3_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta3_1_rat_a) (on_b3_a3) (first_b2_s13))
  :effect (and (last_b3_s13) (first_b3_s13) (not_occupied_ta3_1_rat_a) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (last_b2_s13)) (not (first_b2_s13)) (not (on_b3_a3)) (not (occupied_ta3_1_rat_a)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s13_b0_a1_a3_b2_oc1b_gasoleo_ta1_1_gasoleo_ta3_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta3_1_oc1b) (on_b0_a3) (first_b2_s13))
  :effect (and (last_b0_s13) (first_b0_s13) (not_occupied_ta3_1_oc1b) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (last_b2_s13)) (not (first_b2_s13)) (not (on_b0_a3)) (not (occupied_ta3_1_oc1b)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s12_b1_a1_a2_b3_lco_rat_a_ta1_1_rat_a_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_rat_a) (occupied_ta2_1_lco) (on_b1_a2) (first_b3_s12))
  :effect (and (last_b1_s12) (first_b1_s12) (not_occupied_ta2_1_lco) (on_b3_a1) (occupied_ta1_1_rat_a) (not (last_b3_s12)) (not (first_b3_s12)) (not (on_b1_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_rat_a))))
 (:action pop_unitarypipe_s12_b2_a1_a2_b3_gasoleo_rat_a_ta1_1_rat_a_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_rat_a) (occupied_ta2_1_gasoleo) (on_b2_a2) (first_b3_s12))
  :effect (and (last_b2_s12) (first_b2_s12) (not_occupied_ta2_1_gasoleo) (on_b3_a1) (occupied_ta1_1_rat_a) (not (last_b3_s12)) (not (first_b3_s12)) (not (on_b2_a2)) (not (occupied_ta2_1_gasoleo)) (not (not_occupied_ta1_1_rat_a))))
 (:action pop_unitarypipe_s12_b3_a1_a2_b3_rat_a_rat_a_ta1_1_rat_a_ta2_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta1_1_rat_a) (occupied_ta2_1_rat_a) (on_b3_a2) (first_b3_s12))
  :effect (and (last_b3_s12) (first_b3_s12) (not_occupied_ta2_1_rat_a) (on_b3_a1) (occupied_ta1_1_rat_a) (not (on_b3_a2)) (not (occupied_ta2_1_rat_a)) (not (not_occupied_ta1_1_rat_a))))
 (:action pop_unitarypipe_s13_b4_a1_a3_b3_lco_rat_a_ta1_1_rat_a_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_rat_a) (occupied_ta3_1_lco) (on_b4_a3) (first_b3_s13))
  :effect (and (last_b4_s13) (first_b4_s13) (not_occupied_ta3_1_lco) (on_b3_a1) (occupied_ta1_1_rat_a) (not (last_b3_s13)) (not (first_b3_s13)) (not (on_b4_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_rat_a))))
 (:action pop_unitarypipe_s13_b3_a1_a3_b3_rat_a_rat_a_ta1_1_rat_a_ta3_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta1_1_rat_a) (occupied_ta3_1_rat_a) (on_b3_a3) (first_b3_s13))
  :effect (and (last_b3_s13) (first_b3_s13) (not_occupied_ta3_1_rat_a) (on_b3_a1) (occupied_ta1_1_rat_a) (not (on_b3_a3)) (not (occupied_ta3_1_rat_a)) (not (not_occupied_ta1_1_rat_a))))
 (:action pop_unitarypipe_s12_b1_a1_a2_b5_lco_oca1_ta1_1_oca1_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oca1) (occupied_ta2_1_lco) (on_b1_a2) (first_b5_s12))
  :effect (and (last_b1_s12) (first_b1_s12) (not_occupied_ta2_1_lco) (on_b5_a1) (occupied_ta1_1_oca1) (not (last_b5_s12)) (not (first_b5_s12)) (not (on_b1_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_oca1))))
 (:action pop_unitarypipe_s12_b4_a1_a2_b5_lco_oca1_ta1_1_oca1_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oca1) (occupied_ta2_1_lco) (on_b4_a2) (first_b5_s12))
  :effect (and (last_b4_s12) (first_b4_s12) (not_occupied_ta2_1_lco) (on_b5_a1) (occupied_ta1_1_oca1) (not (last_b5_s12)) (not (first_b5_s12)) (not (on_b4_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_oca1))))
 (:action pop_unitarypipe_s12_b2_a1_a2_b5_gasoleo_oca1_ta1_1_oca1_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oca1) (occupied_ta2_1_gasoleo) (on_b2_a2) (first_b5_s12))
  :effect (and (last_b2_s12) (first_b2_s12) (not_occupied_ta2_1_gasoleo) (on_b5_a1) (occupied_ta1_1_oca1) (not (last_b5_s12)) (not (first_b5_s12)) (not (on_b2_a2)) (not (occupied_ta2_1_gasoleo)) (not (not_occupied_ta1_1_oca1))))
 (:action pop_unitarypipe_s12_b5_a1_a2_b5_oca1_oca1_ta1_1_oca1_ta2_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oca1) (occupied_ta2_1_oca1) (on_b5_a2) (first_b5_s12))
  :effect (and (last_b5_s12) (first_b5_s12) (not_occupied_ta2_1_oca1) (on_b5_a1) (occupied_ta1_1_oca1) (not (on_b5_a2)) (not (occupied_ta2_1_oca1)) (not (not_occupied_ta1_1_oca1))))
 (:action pop_unitarypipe_s12_b0_a1_a2_b5_oc1b_oca1_ta1_1_oca1_ta2_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oca1) (occupied_ta2_1_oc1b) (on_b0_a2) (first_b5_s12))
  :effect (and (last_b0_s12) (first_b0_s12) (not_occupied_ta2_1_oc1b) (on_b5_a1) (occupied_ta1_1_oca1) (not (last_b5_s12)) (not (first_b5_s12)) (not (on_b0_a2)) (not (occupied_ta2_1_oc1b)) (not (not_occupied_ta1_1_oca1))))
 (:action pop_unitarypipe_s13_b4_a1_a3_b5_lco_oca1_ta1_1_oca1_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oca1) (occupied_ta3_1_lco) (on_b4_a3) (first_b5_s13))
  :effect (and (last_b4_s13) (first_b4_s13) (not_occupied_ta3_1_lco) (on_b5_a1) (occupied_ta1_1_oca1) (not (last_b5_s13)) (not (first_b5_s13)) (not (on_b4_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_oca1))))
 (:action pop_unitarypipe_s13_b0_a1_a3_b5_oc1b_oca1_ta1_1_oca1_ta3_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oca1) (occupied_ta3_1_oc1b) (on_b0_a3) (first_b5_s13))
  :effect (and (last_b0_s13) (first_b0_s13) (not_occupied_ta3_1_oc1b) (on_b5_a1) (occupied_ta1_1_oca1) (not (last_b5_s13)) (not (first_b5_s13)) (not (on_b0_a3)) (not (occupied_ta3_1_oc1b)) (not (not_occupied_ta1_1_oca1))))
 (:action pop_unitarypipe_s12_b1_a1_a2_b0_lco_oc1b_ta1_1_oc1b_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oc1b) (occupied_ta2_1_lco) (on_b1_a2) (first_b0_s12))
  :effect (and (last_b1_s12) (first_b1_s12) (not_occupied_ta2_1_lco) (on_b0_a1) (occupied_ta1_1_oc1b) (not (last_b0_s12)) (not (first_b0_s12)) (not (on_b1_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_oc1b))))
 (:action pop_unitarypipe_s12_b2_a1_a2_b0_gasoleo_oc1b_ta1_1_oc1b_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oc1b) (occupied_ta2_1_gasoleo) (on_b2_a2) (first_b0_s12))
  :effect (and (last_b2_s12) (first_b2_s12) (not_occupied_ta2_1_gasoleo) (on_b0_a1) (occupied_ta1_1_oc1b) (not (last_b0_s12)) (not (first_b0_s12)) (not (on_b2_a2)) (not (occupied_ta2_1_gasoleo)) (not (not_occupied_ta1_1_oc1b))))
 (:action pop_unitarypipe_s12_b5_a1_a2_b0_oca1_oc1b_ta1_1_oc1b_ta2_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oc1b) (occupied_ta2_1_oca1) (on_b5_a2) (first_b0_s12))
  :effect (and (last_b5_s12) (first_b5_s12) (not_occupied_ta2_1_oca1) (on_b0_a1) (occupied_ta1_1_oc1b) (not (last_b0_s12)) (not (first_b0_s12)) (not (on_b5_a2)) (not (occupied_ta2_1_oca1)) (not (not_occupied_ta1_1_oc1b))))
 (:action pop_unitarypipe_s12_b0_a1_a2_b0_oc1b_oc1b_ta1_1_oc1b_ta2_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oc1b) (occupied_ta2_1_oc1b) (on_b0_a2) (first_b0_s12))
  :effect (and (last_b0_s12) (first_b0_s12) (not_occupied_ta2_1_oc1b) (on_b0_a1) (occupied_ta1_1_oc1b) (not (on_b0_a2)) (not (occupied_ta2_1_oc1b)) (not (not_occupied_ta1_1_oc1b))))
 (:action pop_unitarypipe_s13_b4_a1_a3_b0_lco_oc1b_ta1_1_oc1b_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oc1b) (occupied_ta3_1_lco) (on_b4_a3) (first_b0_s13))
  :effect (and (last_b4_s13) (first_b4_s13) (not_occupied_ta3_1_lco) (on_b0_a1) (occupied_ta1_1_oc1b) (not (last_b0_s13)) (not (first_b0_s13)) (not (on_b4_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_oc1b))))
 (:action pop_unitarypipe_s13_b0_a1_a3_b0_oc1b_oc1b_ta1_1_oc1b_ta3_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oc1b) (occupied_ta3_1_oc1b) (on_b0_a3) (first_b0_s13))
  :effect (and (last_b0_s13) (first_b0_s13) (not_occupied_ta3_1_oc1b) (on_b0_a1) (occupied_ta1_1_oc1b) (not (on_b0_a3)) (not (occupied_ta3_1_oc1b)) (not (not_occupied_ta1_1_oc1b))))
 (:action push_unitarypipe_s12_b1_a1_a2_b1_lco_lco_ta1_1_lco_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_lco) (on_b1_a1) (first_b1_s12))
  :effect (and (first_b1_s12) (last_b1_s12) (not_occupied_ta1_1_lco) (on_b1_a2) (occupied_ta2_1_lco) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s12_b4_a1_a2_b1_lco_lco_ta1_1_lco_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_lco) (on_b4_a1) (first_b1_s12))
  :effect (and (first_b4_s12) (last_b4_s12) (not_occupied_ta1_1_lco) (on_b1_a2) (occupied_ta2_1_lco) (not (first_b1_s12)) (not (last_b1_s12)) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s12_b1_a1_a2_b4_lco_lco_ta1_1_lco_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_lco) (on_b1_a1) (first_b4_s12))
  :effect (and (first_b1_s12) (last_b1_s12) (not_occupied_ta1_1_lco) (on_b4_a2) (occupied_ta2_1_lco) (not (first_b4_s12)) (not (last_b4_s12)) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s12_b4_a1_a2_b4_lco_lco_ta1_1_lco_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_lco) (on_b4_a1) (first_b4_s12))
  :effect (and (first_b4_s12) (last_b4_s12) (not_occupied_ta1_1_lco) (on_b4_a2) (occupied_ta2_1_lco) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s12_b2_a1_a2_b1_gasoleo_lco_ta1_1_gasoleo_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b1_s12))
  :effect (and (first_b2_s12) (last_b2_s12) (not_occupied_ta1_1_gasoleo) (on_b1_a2) (occupied_ta2_1_lco) (not (first_b1_s12)) (not (last_b1_s12)) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s12_b3_a1_a2_b1_rat_a_lco_ta1_1_rat_a_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_rat_a) (on_b3_a1) (first_b1_s12))
  :effect (and (first_b3_s12) (last_b3_s12) (not_occupied_ta1_1_rat_a) (on_b1_a2) (occupied_ta2_1_lco) (not (first_b1_s12)) (not (last_b1_s12)) (not (on_b3_a1)) (not (occupied_ta1_1_rat_a)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s12_b5_a1_a2_b1_oca1_lco_ta1_1_oca1_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_oca1) (on_b5_a1) (first_b1_s12))
  :effect (and (first_b5_s12) (last_b5_s12) (not_occupied_ta1_1_oca1) (on_b1_a2) (occupied_ta2_1_lco) (not (first_b1_s12)) (not (last_b1_s12)) (not (on_b5_a1)) (not (occupied_ta1_1_oca1)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s12_b5_a1_a2_b4_oca1_lco_ta1_1_oca1_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_oca1) (on_b5_a1) (first_b4_s12))
  :effect (and (first_b5_s12) (last_b5_s12) (not_occupied_ta1_1_oca1) (on_b4_a2) (occupied_ta2_1_lco) (not (first_b4_s12)) (not (last_b4_s12)) (not (on_b5_a1)) (not (occupied_ta1_1_oca1)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s12_b0_a1_a2_b1_oc1b_lco_ta1_1_oc1b_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_oc1b) (on_b0_a1) (first_b1_s12))
  :effect (and (first_b0_s12) (last_b0_s12) (not_occupied_ta1_1_oc1b) (on_b1_a2) (occupied_ta2_1_lco) (not (first_b1_s12)) (not (last_b1_s12)) (not (on_b0_a1)) (not (occupied_ta1_1_oc1b)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s12_b1_a1_a2_b2_lco_gasoleo_ta1_1_lco_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta2_1_gasoleo) (occupied_ta1_1_lco) (on_b1_a1) (first_b2_s12))
  :effect (and (first_b1_s12) (last_b1_s12) (not_occupied_ta1_1_lco) (on_b2_a2) (occupied_ta2_1_gasoleo) (not (first_b2_s12)) (not (last_b2_s12)) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_gasoleo))))
 (:action push_unitarypipe_s12_b4_a1_a2_b2_lco_gasoleo_ta1_1_lco_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta2_1_gasoleo) (occupied_ta1_1_lco) (on_b4_a1) (first_b2_s12))
  :effect (and (first_b4_s12) (last_b4_s12) (not_occupied_ta1_1_lco) (on_b2_a2) (occupied_ta2_1_gasoleo) (not (first_b2_s12)) (not (last_b2_s12)) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_gasoleo))))
 (:action push_unitarypipe_s12_b2_a1_a2_b2_gasoleo_gasoleo_ta1_1_gasoleo_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta2_1_gasoleo) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b2_s12))
  :effect (and (first_b2_s12) (last_b2_s12) (not_occupied_ta1_1_gasoleo) (on_b2_a2) (occupied_ta2_1_gasoleo) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta2_1_gasoleo))))
 (:action push_unitarypipe_s12_b3_a1_a2_b2_rat_a_gasoleo_ta1_1_rat_a_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta2_1_gasoleo) (occupied_ta1_1_rat_a) (on_b3_a1) (first_b2_s12))
  :effect (and (first_b3_s12) (last_b3_s12) (not_occupied_ta1_1_rat_a) (on_b2_a2) (occupied_ta2_1_gasoleo) (not (first_b2_s12)) (not (last_b2_s12)) (not (on_b3_a1)) (not (occupied_ta1_1_rat_a)) (not (not_occupied_ta2_1_gasoleo))))
 (:action push_unitarypipe_s12_b5_a1_a2_b2_oca1_gasoleo_ta1_1_oca1_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta2_1_gasoleo) (occupied_ta1_1_oca1) (on_b5_a1) (first_b2_s12))
  :effect (and (first_b5_s12) (last_b5_s12) (not_occupied_ta1_1_oca1) (on_b2_a2) (occupied_ta2_1_gasoleo) (not (first_b2_s12)) (not (last_b2_s12)) (not (on_b5_a1)) (not (occupied_ta1_1_oca1)) (not (not_occupied_ta2_1_gasoleo))))
 (:action push_unitarypipe_s12_b0_a1_a2_b2_oc1b_gasoleo_ta1_1_oc1b_ta2_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta2_1_gasoleo) (occupied_ta1_1_oc1b) (on_b0_a1) (first_b2_s12))
  :effect (and (first_b0_s12) (last_b0_s12) (not_occupied_ta1_1_oc1b) (on_b2_a2) (occupied_ta2_1_gasoleo) (not (first_b2_s12)) (not (last_b2_s12)) (not (on_b0_a1)) (not (occupied_ta1_1_oc1b)) (not (not_occupied_ta2_1_gasoleo))))
 (:action push_unitarypipe_s12_b1_a1_a2_b3_lco_rat_a_ta1_1_lco_ta2_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta2_1_rat_a) (occupied_ta1_1_lco) (on_b1_a1) (first_b3_s12))
  :effect (and (first_b1_s12) (last_b1_s12) (not_occupied_ta1_1_lco) (on_b3_a2) (occupied_ta2_1_rat_a) (not (first_b3_s12)) (not (last_b3_s12)) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_rat_a))))
 (:action push_unitarypipe_s12_b4_a1_a2_b3_lco_rat_a_ta1_1_lco_ta2_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta2_1_rat_a) (occupied_ta1_1_lco) (on_b4_a1) (first_b3_s12))
  :effect (and (first_b4_s12) (last_b4_s12) (not_occupied_ta1_1_lco) (on_b3_a2) (occupied_ta2_1_rat_a) (not (first_b3_s12)) (not (last_b3_s12)) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_rat_a))))
 (:action push_unitarypipe_s12_b2_a1_a2_b3_gasoleo_rat_a_ta1_1_gasoleo_ta2_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta2_1_rat_a) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b3_s12))
  :effect (and (first_b2_s12) (last_b2_s12) (not_occupied_ta1_1_gasoleo) (on_b3_a2) (occupied_ta2_1_rat_a) (not (first_b3_s12)) (not (last_b3_s12)) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta2_1_rat_a))))
 (:action push_unitarypipe_s12_b3_a1_a2_b3_rat_a_rat_a_ta1_1_rat_a_ta2_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta2_1_rat_a) (occupied_ta1_1_rat_a) (on_b3_a1) (first_b3_s12))
  :effect (and (first_b3_s12) (last_b3_s12) (not_occupied_ta1_1_rat_a) (on_b3_a2) (occupied_ta2_1_rat_a) (not (on_b3_a1)) (not (occupied_ta1_1_rat_a)) (not (not_occupied_ta2_1_rat_a))))
 (:action push_unitarypipe_s12_b1_a1_a2_b5_lco_oca1_ta1_1_lco_ta2_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta2_1_oca1) (occupied_ta1_1_lco) (on_b1_a1) (first_b5_s12))
  :effect (and (first_b1_s12) (last_b1_s12) (not_occupied_ta1_1_lco) (on_b5_a2) (occupied_ta2_1_oca1) (not (first_b5_s12)) (not (last_b5_s12)) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_oca1))))
 (:action push_unitarypipe_s12_b4_a1_a2_b5_lco_oca1_ta1_1_lco_ta2_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta2_1_oca1) (occupied_ta1_1_lco) (on_b4_a1) (first_b5_s12))
  :effect (and (first_b4_s12) (last_b4_s12) (not_occupied_ta1_1_lco) (on_b5_a2) (occupied_ta2_1_oca1) (not (first_b5_s12)) (not (last_b5_s12)) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_oca1))))
 (:action push_unitarypipe_s12_b2_a1_a2_b5_gasoleo_oca1_ta1_1_gasoleo_ta2_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta2_1_oca1) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b5_s12))
  :effect (and (first_b2_s12) (last_b2_s12) (not_occupied_ta1_1_gasoleo) (on_b5_a2) (occupied_ta2_1_oca1) (not (first_b5_s12)) (not (last_b5_s12)) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta2_1_oca1))))
 (:action push_unitarypipe_s12_b5_a1_a2_b5_oca1_oca1_ta1_1_oca1_ta2_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta2_1_oca1) (occupied_ta1_1_oca1) (on_b5_a1) (first_b5_s12))
  :effect (and (first_b5_s12) (last_b5_s12) (not_occupied_ta1_1_oca1) (on_b5_a2) (occupied_ta2_1_oca1) (not (on_b5_a1)) (not (occupied_ta1_1_oca1)) (not (not_occupied_ta2_1_oca1))))
 (:action push_unitarypipe_s12_b0_a1_a2_b5_oc1b_oca1_ta1_1_oc1b_ta2_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta2_1_oca1) (occupied_ta1_1_oc1b) (on_b0_a1) (first_b5_s12))
  :effect (and (first_b0_s12) (last_b0_s12) (not_occupied_ta1_1_oc1b) (on_b5_a2) (occupied_ta2_1_oca1) (not (first_b5_s12)) (not (last_b5_s12)) (not (on_b0_a1)) (not (occupied_ta1_1_oc1b)) (not (not_occupied_ta2_1_oca1))))
 (:action push_unitarypipe_s12_b1_a1_a2_b0_lco_oc1b_ta1_1_lco_ta2_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta2_1_oc1b) (occupied_ta1_1_lco) (on_b1_a1) (first_b0_s12))
  :effect (and (first_b1_s12) (last_b1_s12) (not_occupied_ta1_1_lco) (on_b0_a2) (occupied_ta2_1_oc1b) (not (first_b0_s12)) (not (last_b0_s12)) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_oc1b))))
 (:action push_unitarypipe_s12_b4_a1_a2_b0_lco_oc1b_ta1_1_lco_ta2_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta2_1_oc1b) (occupied_ta1_1_lco) (on_b4_a1) (first_b0_s12))
  :effect (and (first_b4_s12) (last_b4_s12) (not_occupied_ta1_1_lco) (on_b0_a2) (occupied_ta2_1_oc1b) (not (first_b0_s12)) (not (last_b0_s12)) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta2_1_oc1b))))
 (:action push_unitarypipe_s12_b2_a1_a2_b0_gasoleo_oc1b_ta1_1_gasoleo_ta2_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta2_1_oc1b) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b0_s12))
  :effect (and (first_b2_s12) (last_b2_s12) (not_occupied_ta1_1_gasoleo) (on_b0_a2) (occupied_ta2_1_oc1b) (not (first_b0_s12)) (not (last_b0_s12)) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta2_1_oc1b))))
 (:action push_unitarypipe_s12_b5_a1_a2_b0_oca1_oc1b_ta1_1_oca1_ta2_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta2_1_oc1b) (occupied_ta1_1_oca1) (on_b5_a1) (first_b0_s12))
  :effect (and (first_b5_s12) (last_b5_s12) (not_occupied_ta1_1_oca1) (on_b0_a2) (occupied_ta2_1_oc1b) (not (first_b0_s12)) (not (last_b0_s12)) (not (on_b5_a1)) (not (occupied_ta1_1_oca1)) (not (not_occupied_ta2_1_oc1b))))
 (:action push_unitarypipe_s12_b0_a1_a2_b0_oc1b_oc1b_ta1_1_oc1b_ta2_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta2_1_oc1b) (occupied_ta1_1_oc1b) (on_b0_a1) (first_b0_s12))
  :effect (and (first_b0_s12) (last_b0_s12) (not_occupied_ta1_1_oc1b) (on_b0_a2) (occupied_ta2_1_oc1b) (not (on_b0_a1)) (not (occupied_ta1_1_oc1b)) (not (not_occupied_ta2_1_oc1b))))
 (:action push_unitarypipe_s13_b1_a1_a3_b1_lco_lco_ta1_1_lco_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_lco) (on_b1_a1) (first_b1_s13))
  :effect (and (first_b1_s13) (last_b1_s13) (not_occupied_ta1_1_lco) (on_b1_a3) (occupied_ta3_1_lco) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b4_a1_a3_b1_lco_lco_ta1_1_lco_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_lco) (on_b4_a1) (first_b1_s13))
  :effect (and (first_b4_s13) (last_b4_s13) (not_occupied_ta1_1_lco) (on_b1_a3) (occupied_ta3_1_lco) (not (first_b1_s13)) (not (last_b1_s13)) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b1_a1_a3_b4_lco_lco_ta1_1_lco_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_lco) (on_b1_a1) (first_b4_s13))
  :effect (and (first_b1_s13) (last_b1_s13) (not_occupied_ta1_1_lco) (on_b4_a3) (occupied_ta3_1_lco) (not (first_b4_s13)) (not (last_b4_s13)) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b4_a1_a3_b4_lco_lco_ta1_1_lco_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_lco) (on_b4_a1) (first_b4_s13))
  :effect (and (first_b4_s13) (last_b4_s13) (not_occupied_ta1_1_lco) (on_b4_a3) (occupied_ta3_1_lco) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b2_a1_a3_b1_gasoleo_lco_ta1_1_gasoleo_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b1_s13))
  :effect (and (first_b2_s13) (last_b2_s13) (not_occupied_ta1_1_gasoleo) (on_b1_a3) (occupied_ta3_1_lco) (not (first_b1_s13)) (not (last_b1_s13)) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b2_a1_a3_b4_gasoleo_lco_ta1_1_gasoleo_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b4_s13))
  :effect (and (first_b2_s13) (last_b2_s13) (not_occupied_ta1_1_gasoleo) (on_b4_a3) (occupied_ta3_1_lco) (not (first_b4_s13)) (not (last_b4_s13)) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b3_a1_a3_b1_rat_a_lco_ta1_1_rat_a_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_rat_a) (on_b3_a1) (first_b1_s13))
  :effect (and (first_b3_s13) (last_b3_s13) (not_occupied_ta1_1_rat_a) (on_b1_a3) (occupied_ta3_1_lco) (not (first_b1_s13)) (not (last_b1_s13)) (not (on_b3_a1)) (not (occupied_ta1_1_rat_a)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b3_a1_a3_b4_rat_a_lco_ta1_1_rat_a_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_rat_a) (on_b3_a1) (first_b4_s13))
  :effect (and (first_b3_s13) (last_b3_s13) (not_occupied_ta1_1_rat_a) (on_b4_a3) (occupied_ta3_1_lco) (not (first_b4_s13)) (not (last_b4_s13)) (not (on_b3_a1)) (not (occupied_ta1_1_rat_a)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b5_a1_a3_b1_oca1_lco_ta1_1_oca1_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_oca1) (on_b5_a1) (first_b1_s13))
  :effect (and (first_b5_s13) (last_b5_s13) (not_occupied_ta1_1_oca1) (on_b1_a3) (occupied_ta3_1_lco) (not (first_b1_s13)) (not (last_b1_s13)) (not (on_b5_a1)) (not (occupied_ta1_1_oca1)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b5_a1_a3_b4_oca1_lco_ta1_1_oca1_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_oca1) (on_b5_a1) (first_b4_s13))
  :effect (and (first_b5_s13) (last_b5_s13) (not_occupied_ta1_1_oca1) (on_b4_a3) (occupied_ta3_1_lco) (not (first_b4_s13)) (not (last_b4_s13)) (not (on_b5_a1)) (not (occupied_ta1_1_oca1)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b0_a1_a3_b1_oc1b_lco_ta1_1_oc1b_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_oc1b) (on_b0_a1) (first_b1_s13))
  :effect (and (first_b0_s13) (last_b0_s13) (not_occupied_ta1_1_oc1b) (on_b1_a3) (occupied_ta3_1_lco) (not (first_b1_s13)) (not (last_b1_s13)) (not (on_b0_a1)) (not (occupied_ta1_1_oc1b)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b0_a1_a3_b4_oc1b_lco_ta1_1_oc1b_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta3_1_lco) (occupied_ta1_1_oc1b) (on_b0_a1) (first_b4_s13))
  :effect (and (first_b0_s13) (last_b0_s13) (not_occupied_ta1_1_oc1b) (on_b4_a3) (occupied_ta3_1_lco) (not (first_b4_s13)) (not (last_b4_s13)) (not (on_b0_a1)) (not (occupied_ta1_1_oc1b)) (not (not_occupied_ta3_1_lco))))
 (:action push_unitarypipe_s13_b1_a1_a3_b2_lco_gasoleo_ta1_1_lco_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta3_1_gasoleo) (occupied_ta1_1_lco) (on_b1_a1) (first_b2_s13))
  :effect (and (first_b1_s13) (last_b1_s13) (not_occupied_ta1_1_lco) (on_b2_a3) (occupied_ta3_1_gasoleo) (not (first_b2_s13)) (not (last_b2_s13)) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_gasoleo))))
 (:action push_unitarypipe_s13_b4_a1_a3_b2_lco_gasoleo_ta1_1_lco_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta3_1_gasoleo) (occupied_ta1_1_lco) (on_b4_a1) (first_b2_s13))
  :effect (and (first_b4_s13) (last_b4_s13) (not_occupied_ta1_1_lco) (on_b2_a3) (occupied_ta3_1_gasoleo) (not (first_b2_s13)) (not (last_b2_s13)) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_gasoleo))))
 (:action push_unitarypipe_s13_b5_a1_a3_b2_oca1_gasoleo_ta1_1_oca1_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta3_1_gasoleo) (occupied_ta1_1_oca1) (on_b5_a1) (first_b2_s13))
  :effect (and (first_b5_s13) (last_b5_s13) (not_occupied_ta1_1_oca1) (on_b2_a3) (occupied_ta3_1_gasoleo) (not (first_b2_s13)) (not (last_b2_s13)) (not (on_b5_a1)) (not (occupied_ta1_1_oca1)) (not (not_occupied_ta3_1_gasoleo))))
 (:action push_unitarypipe_s13_b1_a1_a3_b3_lco_rat_a_ta1_1_lco_ta3_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta3_1_rat_a) (occupied_ta1_1_lco) (on_b1_a1) (first_b3_s13))
  :effect (and (first_b1_s13) (last_b1_s13) (not_occupied_ta1_1_lco) (on_b3_a3) (occupied_ta3_1_rat_a) (not (first_b3_s13)) (not (last_b3_s13)) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_rat_a))))
 (:action push_unitarypipe_s13_b4_a1_a3_b3_lco_rat_a_ta1_1_lco_ta3_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta3_1_rat_a) (occupied_ta1_1_lco) (on_b4_a1) (first_b3_s13))
  :effect (and (first_b4_s13) (last_b4_s13) (not_occupied_ta1_1_lco) (on_b3_a3) (occupied_ta3_1_rat_a) (not (first_b3_s13)) (not (last_b3_s13)) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_rat_a))))
 (:action push_unitarypipe_s13_b2_a1_a3_b3_gasoleo_rat_a_ta1_1_gasoleo_ta3_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta3_1_rat_a) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b3_s13))
  :effect (and (first_b2_s13) (last_b2_s13) (not_occupied_ta1_1_gasoleo) (on_b3_a3) (occupied_ta3_1_rat_a) (not (first_b3_s13)) (not (last_b3_s13)) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta3_1_rat_a))))
 (:action push_unitarypipe_s13_b3_a1_a3_b3_rat_a_rat_a_ta1_1_rat_a_ta3_1_rat_a
  :parameters ()
  :precondition (and (not_occupied_ta3_1_rat_a) (occupied_ta1_1_rat_a) (on_b3_a1) (first_b3_s13))
  :effect (and (first_b3_s13) (last_b3_s13) (not_occupied_ta1_1_rat_a) (on_b3_a3) (occupied_ta3_1_rat_a) (not (on_b3_a1)) (not (occupied_ta1_1_rat_a)) (not (not_occupied_ta3_1_rat_a))))
 (:action push_unitarypipe_s13_b1_a1_a3_b5_lco_oca1_ta1_1_lco_ta3_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta3_1_oca1) (occupied_ta1_1_lco) (on_b1_a1) (first_b5_s13))
  :effect (and (first_b1_s13) (last_b1_s13) (not_occupied_ta1_1_lco) (on_b5_a3) (occupied_ta3_1_oca1) (not (first_b5_s13)) (not (last_b5_s13)) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_oca1))))
 (:action push_unitarypipe_s13_b4_a1_a3_b5_lco_oca1_ta1_1_lco_ta3_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta3_1_oca1) (occupied_ta1_1_lco) (on_b4_a1) (first_b5_s13))
  :effect (and (first_b4_s13) (last_b4_s13) (not_occupied_ta1_1_lco) (on_b5_a3) (occupied_ta3_1_oca1) (not (first_b5_s13)) (not (last_b5_s13)) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_oca1))))
 (:action push_unitarypipe_s13_b5_a1_a3_b5_oca1_oca1_ta1_1_oca1_ta3_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta3_1_oca1) (occupied_ta1_1_oca1) (on_b5_a1) (first_b5_s13))
  :effect (and (first_b5_s13) (last_b5_s13) (not_occupied_ta1_1_oca1) (on_b5_a3) (occupied_ta3_1_oca1) (not (on_b5_a1)) (not (occupied_ta1_1_oca1)) (not (not_occupied_ta3_1_oca1))))
 (:action push_unitarypipe_s13_b1_a1_a3_b0_lco_oc1b_ta1_1_lco_ta3_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta3_1_oc1b) (occupied_ta1_1_lco) (on_b1_a1) (first_b0_s13))
  :effect (and (first_b1_s13) (last_b1_s13) (not_occupied_ta1_1_lco) (on_b0_a3) (occupied_ta3_1_oc1b) (not (first_b0_s13)) (not (last_b0_s13)) (not (on_b1_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_oc1b))))
 (:action push_unitarypipe_s13_b4_a1_a3_b0_lco_oc1b_ta1_1_lco_ta3_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta3_1_oc1b) (occupied_ta1_1_lco) (on_b4_a1) (first_b0_s13))
  :effect (and (first_b4_s13) (last_b4_s13) (not_occupied_ta1_1_lco) (on_b0_a3) (occupied_ta3_1_oc1b) (not (first_b0_s13)) (not (last_b0_s13)) (not (on_b4_a1)) (not (occupied_ta1_1_lco)) (not (not_occupied_ta3_1_oc1b))))
 (:action push_unitarypipe_s13_b2_a1_a3_b0_gasoleo_oc1b_ta1_1_gasoleo_ta3_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta3_1_oc1b) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b0_s13))
  :effect (and (first_b2_s13) (last_b2_s13) (not_occupied_ta1_1_gasoleo) (on_b0_a3) (occupied_ta3_1_oc1b) (not (first_b0_s13)) (not (last_b0_s13)) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta3_1_oc1b))))
 (:action push_unitarypipe_s13_b5_a1_a3_b0_oca1_oc1b_ta1_1_oca1_ta3_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta3_1_oc1b) (occupied_ta1_1_oca1) (on_b5_a1) (first_b0_s13))
  :effect (and (first_b5_s13) (last_b5_s13) (not_occupied_ta1_1_oca1) (on_b0_a3) (occupied_ta3_1_oc1b) (not (first_b0_s13)) (not (last_b0_s13)) (not (on_b5_a1)) (not (occupied_ta1_1_oca1)) (not (not_occupied_ta3_1_oc1b))))
 (:action push_unitarypipe_s13_b0_a1_a3_b0_oc1b_oc1b_ta1_1_oc1b_ta3_1_oc1b
  :parameters ()
  :precondition (and (not_occupied_ta3_1_oc1b) (occupied_ta1_1_oc1b) (on_b0_a1) (first_b0_s13))
  :effect (and (first_b0_s13) (last_b0_s13) (not_occupied_ta1_1_oc1b) (on_b0_a3) (occupied_ta3_1_oc1b) (not (on_b0_a1)) (not (occupied_ta1_1_oc1b)) (not (not_occupied_ta3_1_oc1b))))
 (:action pop_unitarypipe_s12_b4_a1_a2_b4_lco_lco_ta1_1_lco_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta2_1_lco) (on_b4_a2) (first_b4_s12))
  :effect (and (last_b4_s12) (first_b4_s12) (not_occupied_ta2_1_lco) (on_b4_a1) (occupied_ta1_1_lco) (not (on_b4_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b1_a1_a3_b1_lco_lco_ta1_1_lco_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_lco) (on_b1_a3) (first_b1_s13))
  :effect (and (last_b1_s13) (first_b1_s13) (not_occupied_ta3_1_lco) (on_b1_a1) (occupied_ta1_1_lco) (not (on_b1_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b2_a1_a3_b1_gasoleo_lco_ta1_1_lco_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_gasoleo) (on_b2_a3) (first_b1_s13))
  :effect (and (last_b2_s13) (first_b2_s13) (not_occupied_ta3_1_gasoleo) (on_b1_a1) (occupied_ta1_1_lco) (not (last_b1_s13)) (not (first_b1_s13)) (not (on_b2_a3)) (not (occupied_ta3_1_gasoleo)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s13_b5_a1_a3_b1_oca1_lco_ta1_1_lco_ta3_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta1_1_lco) (occupied_ta3_1_oca1) (on_b5_a3) (first_b1_s13))
  :effect (and (last_b5_s13) (first_b5_s13) (not_occupied_ta3_1_oca1) (on_b1_a1) (occupied_ta1_1_lco) (not (last_b1_s13)) (not (first_b1_s13)) (not (on_b5_a3)) (not (occupied_ta3_1_oca1)) (not (not_occupied_ta1_1_lco))))
 (:action pop_unitarypipe_s12_b4_a1_a2_b2_lco_gasoleo_ta1_1_gasoleo_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta2_1_lco) (on_b4_a2) (first_b2_s12))
  :effect (and (last_b4_s12) (first_b4_s12) (not_occupied_ta2_1_lco) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (last_b2_s12)) (not (first_b2_s12)) (not (on_b4_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s13_b1_a1_a3_b2_lco_gasoleo_ta1_1_gasoleo_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta3_1_lco) (on_b1_a3) (first_b2_s13))
  :effect (and (last_b1_s13) (first_b1_s13) (not_occupied_ta3_1_lco) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (last_b2_s13)) (not (first_b2_s13)) (not (on_b1_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s13_b2_a1_a3_b2_gasoleo_gasoleo_ta1_1_gasoleo_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta3_1_gasoleo) (on_b2_a3) (first_b2_s13))
  :effect (and (last_b2_s13) (first_b2_s13) (not_occupied_ta3_1_gasoleo) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (on_b2_a3)) (not (occupied_ta3_1_gasoleo)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s13_b5_a1_a3_b2_oca1_gasoleo_ta1_1_gasoleo_ta3_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta1_1_gasoleo) (occupied_ta3_1_oca1) (on_b5_a3) (first_b2_s13))
  :effect (and (last_b5_s13) (first_b5_s13) (not_occupied_ta3_1_oca1) (on_b2_a1) (occupied_ta1_1_gasoleo) (not (last_b2_s13)) (not (first_b2_s13)) (not (on_b5_a3)) (not (occupied_ta3_1_oca1)) (not (not_occupied_ta1_1_gasoleo))))
 (:action pop_unitarypipe_s12_b4_a1_a2_b3_lco_rat_a_ta1_1_rat_a_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_rat_a) (occupied_ta2_1_lco) (on_b4_a2) (first_b3_s12))
  :effect (and (last_b4_s12) (first_b4_s12) (not_occupied_ta2_1_lco) (on_b3_a1) (occupied_ta1_1_rat_a) (not (last_b3_s12)) (not (first_b3_s12)) (not (on_b4_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_rat_a))))
 (:action pop_unitarypipe_s13_b1_a1_a3_b3_lco_rat_a_ta1_1_rat_a_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_rat_a) (occupied_ta3_1_lco) (on_b1_a3) (first_b3_s13))
  :effect (and (last_b1_s13) (first_b1_s13) (not_occupied_ta3_1_lco) (on_b3_a1) (occupied_ta1_1_rat_a) (not (last_b3_s13)) (not (first_b3_s13)) (not (on_b1_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_rat_a))))
 (:action pop_unitarypipe_s13_b2_a1_a3_b3_gasoleo_rat_a_ta1_1_rat_a_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_rat_a) (occupied_ta3_1_gasoleo) (on_b2_a3) (first_b3_s13))
  :effect (and (last_b2_s13) (first_b2_s13) (not_occupied_ta3_1_gasoleo) (on_b3_a1) (occupied_ta1_1_rat_a) (not (last_b3_s13)) (not (first_b3_s13)) (not (on_b2_a3)) (not (occupied_ta3_1_gasoleo)) (not (not_occupied_ta1_1_rat_a))))
 (:action pop_unitarypipe_s13_b1_a1_a3_b5_lco_oca1_ta1_1_oca1_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oca1) (occupied_ta3_1_lco) (on_b1_a3) (first_b5_s13))
  :effect (and (last_b1_s13) (first_b1_s13) (not_occupied_ta3_1_lco) (on_b5_a1) (occupied_ta1_1_oca1) (not (last_b5_s13)) (not (first_b5_s13)) (not (on_b1_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_oca1))))
 (:action pop_unitarypipe_s13_b2_a1_a3_b5_gasoleo_oca1_ta1_1_oca1_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oca1) (occupied_ta3_1_gasoleo) (on_b2_a3) (first_b5_s13))
  :effect (and (last_b2_s13) (first_b2_s13) (not_occupied_ta3_1_gasoleo) (on_b5_a1) (occupied_ta1_1_oca1) (not (last_b5_s13)) (not (first_b5_s13)) (not (on_b2_a3)) (not (occupied_ta3_1_gasoleo)) (not (not_occupied_ta1_1_oca1))))
 (:action pop_unitarypipe_s13_b5_a1_a3_b5_oca1_oca1_ta1_1_oca1_ta3_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oca1) (occupied_ta3_1_oca1) (on_b5_a3) (first_b5_s13))
  :effect (and (last_b5_s13) (first_b5_s13) (not_occupied_ta3_1_oca1) (on_b5_a1) (occupied_ta1_1_oca1) (not (on_b5_a3)) (not (occupied_ta3_1_oca1)) (not (not_occupied_ta1_1_oca1))))
 (:action pop_unitarypipe_s12_b4_a1_a2_b0_lco_oc1b_ta1_1_oc1b_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oc1b) (occupied_ta2_1_lco) (on_b4_a2) (first_b0_s12))
  :effect (and (last_b4_s12) (first_b4_s12) (not_occupied_ta2_1_lco) (on_b0_a1) (occupied_ta1_1_oc1b) (not (last_b0_s12)) (not (first_b0_s12)) (not (on_b4_a2)) (not (occupied_ta2_1_lco)) (not (not_occupied_ta1_1_oc1b))))
 (:action pop_unitarypipe_s13_b1_a1_a3_b0_lco_oc1b_ta1_1_oc1b_ta3_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oc1b) (occupied_ta3_1_lco) (on_b1_a3) (first_b0_s13))
  :effect (and (last_b1_s13) (first_b1_s13) (not_occupied_ta3_1_lco) (on_b0_a1) (occupied_ta1_1_oc1b) (not (last_b0_s13)) (not (first_b0_s13)) (not (on_b1_a3)) (not (occupied_ta3_1_lco)) (not (not_occupied_ta1_1_oc1b))))
 (:action pop_unitarypipe_s13_b2_a1_a3_b0_gasoleo_oc1b_ta1_1_oc1b_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oc1b) (occupied_ta3_1_gasoleo) (on_b2_a3) (first_b0_s13))
  :effect (and (last_b2_s13) (first_b2_s13) (not_occupied_ta3_1_gasoleo) (on_b0_a1) (occupied_ta1_1_oc1b) (not (last_b0_s13)) (not (first_b0_s13)) (not (on_b2_a3)) (not (occupied_ta3_1_gasoleo)) (not (not_occupied_ta1_1_oc1b))))
 (:action pop_unitarypipe_s13_b5_a1_a3_b0_oca1_oc1b_ta1_1_oc1b_ta3_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta1_1_oc1b) (occupied_ta3_1_oca1) (on_b5_a3) (first_b0_s13))
  :effect (and (last_b5_s13) (first_b5_s13) (not_occupied_ta3_1_oca1) (on_b0_a1) (occupied_ta1_1_oc1b) (not (last_b0_s13)) (not (first_b0_s13)) (not (on_b5_a3)) (not (occupied_ta3_1_oca1)) (not (not_occupied_ta1_1_oc1b))))
 (:action push_unitarypipe_s12_b2_a1_a2_b4_gasoleo_lco_ta1_1_gasoleo_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b4_s12))
  :effect (and (first_b2_s12) (last_b2_s12) (not_occupied_ta1_1_gasoleo) (on_b4_a2) (occupied_ta2_1_lco) (not (first_b4_s12)) (not (last_b4_s12)) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s12_b3_a1_a2_b4_rat_a_lco_ta1_1_rat_a_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_rat_a) (on_b3_a1) (first_b4_s12))
  :effect (and (first_b3_s12) (last_b3_s12) (not_occupied_ta1_1_rat_a) (on_b4_a2) (occupied_ta2_1_lco) (not (first_b4_s12)) (not (last_b4_s12)) (not (on_b3_a1)) (not (occupied_ta1_1_rat_a)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s12_b0_a1_a2_b4_oc1b_lco_ta1_1_oc1b_ta2_1_lco
  :parameters ()
  :precondition (and (not_occupied_ta2_1_lco) (occupied_ta1_1_oc1b) (on_b0_a1) (first_b4_s12))
  :effect (and (first_b0_s12) (last_b0_s12) (not_occupied_ta1_1_oc1b) (on_b4_a2) (occupied_ta2_1_lco) (not (first_b4_s12)) (not (last_b4_s12)) (not (on_b0_a1)) (not (occupied_ta1_1_oc1b)) (not (not_occupied_ta2_1_lco))))
 (:action push_unitarypipe_s13_b2_a1_a3_b2_gasoleo_gasoleo_ta1_1_gasoleo_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta3_1_gasoleo) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b2_s13))
  :effect (and (first_b2_s13) (last_b2_s13) (not_occupied_ta1_1_gasoleo) (on_b2_a3) (occupied_ta3_1_gasoleo) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta3_1_gasoleo))))
 (:action push_unitarypipe_s13_b3_a1_a3_b2_rat_a_gasoleo_ta1_1_rat_a_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta3_1_gasoleo) (occupied_ta1_1_rat_a) (on_b3_a1) (first_b2_s13))
  :effect (and (first_b3_s13) (last_b3_s13) (not_occupied_ta1_1_rat_a) (on_b2_a3) (occupied_ta3_1_gasoleo) (not (first_b2_s13)) (not (last_b2_s13)) (not (on_b3_a1)) (not (occupied_ta1_1_rat_a)) (not (not_occupied_ta3_1_gasoleo))))
 (:action push_unitarypipe_s13_b0_a1_a3_b2_oc1b_gasoleo_ta1_1_oc1b_ta3_1_gasoleo
  :parameters ()
  :precondition (and (not_occupied_ta3_1_gasoleo) (occupied_ta1_1_oc1b) (on_b0_a1) (first_b2_s13))
  :effect (and (first_b0_s13) (last_b0_s13) (not_occupied_ta1_1_oc1b) (on_b2_a3) (occupied_ta3_1_gasoleo) (not (first_b2_s13)) (not (last_b2_s13)) (not (on_b0_a1)) (not (occupied_ta1_1_oc1b)) (not (not_occupied_ta3_1_gasoleo))))
 (:action push_unitarypipe_s13_b2_a1_a3_b5_gasoleo_oca1_ta1_1_gasoleo_ta3_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta3_1_oca1) (occupied_ta1_1_gasoleo) (on_b2_a1) (first_b5_s13))
  :effect (and (first_b2_s13) (last_b2_s13) (not_occupied_ta1_1_gasoleo) (on_b5_a3) (occupied_ta3_1_oca1) (not (first_b5_s13)) (not (last_b5_s13)) (not (on_b2_a1)) (not (occupied_ta1_1_gasoleo)) (not (not_occupied_ta3_1_oca1))))
 (:action push_unitarypipe_s13_b0_a1_a3_b5_oc1b_oca1_ta1_1_oc1b_ta3_1_oca1
  :parameters ()
  :precondition (and (not_occupied_ta3_1_oca1) (occupied_ta1_1_oc1b) (on_b0_a1) (first_b5_s13))
  :effect (and (first_b0_s13) (last_b0_s13) (not_occupied_ta1_1_oc1b) (on_b5_a3) (occupied_ta3_1_oca1) (not (first_b5_s13)) (not (last_b5_s13)) (not (on_b0_a1)) (not (occupied_ta1_1_oc1b)) (not (not_occupied_ta3_1_oca1))))
)
