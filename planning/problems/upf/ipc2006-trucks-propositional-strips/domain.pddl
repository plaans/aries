(define (domain grounded_truck_1-domain)
 (:requirements :strips)
 (:predicates (foo) (time_now_t1) (at_truck1_l2) (at_truck1_l1) (delivered_package1_l2_t6) (at_destination_package1_l2) (delivered_package2_l2_t6) (at_destination_package2_l2) (delivered_package3_l2_t6) (at_destination_package3_l2) (delivered_package1_l2_t5) (delivered_package2_l2_t5) (delivered_package3_l2_t5) (delivered_package1_l2_t4) (delivered_package2_l2_t4) (delivered_package3_l2_t4) (delivered_package1_l2_t3) (delivered_package2_l2_t3) (delivered_package3_l2_t3) (delivered_package1_l2_t2) (delivered_package2_l2_t2) (delivered_package3_l2_t2) (delivered_package1_l2_t1) (delivered_package2_l2_t1) (delivered_package3_l2_t1) (in_package1_truck1_a1) (in_package1_truck1_a2) (in_package2_truck1_a1) (in_package2_truck1_a2) (in_package3_truck1_a1) (in_package3_truck1_a2) (at_package1_l1) (at_package1_l3) (at_package2_l1) (at_package2_l3) (at_package3_l1) (at_package3_l3) (time_now_t2) (delivered_package1_l1_t6) (at_destination_package1_l1) (delivered_package1_l3_t6) (at_destination_package1_l3) (delivered_package2_l1_t6) (at_destination_package2_l1) (delivered_package2_l3_t6) (at_destination_package2_l3) (delivered_package3_l1_t6) (at_destination_package3_l1) (delivered_package3_l3_t6) (at_destination_package3_l3) (delivered_package1_l1_t5) (delivered_package1_l3_t5) (delivered_package2_l1_t5) (delivered_package2_l3_t5) (delivered_package3_l1_t5) (delivered_package3_l3_t5) (delivered_package1_l1_t4) (delivered_package1_l3_t4) (delivered_package2_l1_t4) (delivered_package2_l3_t4) (delivered_package3_l1_t4) (delivered_package3_l3_t4) (delivered_package1_l1_t3) (delivered_package1_l3_t3) (delivered_package2_l1_t3) (delivered_package2_l3_t3) (delivered_package3_l1_t3) (delivered_package3_l3_t3) (delivered_package1_l1_t2) (delivered_package1_l3_t2) (delivered_package2_l1_t2) (delivered_package2_l3_t2) (delivered_package3_l1_t2) (delivered_package3_l3_t2) (delivered_package1_l1_t1) (delivered_package1_l3_t1) (delivered_package2_l1_t1) (delivered_package2_l3_t1) (delivered_package3_l1_t1) (delivered_package3_l3_t1) (time_now_t3) (time_now_t4) (time_now_t5) (time_now_t6) (at_package3_l2) (at_package2_l2) (at_package1_l2) (at_truck1_l3) (free_a2_truck1) (free_a1_truck1) (time_now_t0))
 (:action deliver_package3_l3_t6_t6
  :parameters ()
  :precondition (and (time_now_t6) (at_package3_l3))
  :effect (and (delivered_package3_l3_t6) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t6_t6
  :parameters ()
  :precondition (and (time_now_t6) (at_package3_l2))
  :effect (and (delivered_package3_l2_t6) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t6_t6
  :parameters ()
  :precondition (and (time_now_t6) (at_package3_l1))
  :effect (and (delivered_package3_l1_t6) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t6_t6
  :parameters ()
  :precondition (and (time_now_t6) (at_package2_l3))
  :effect (and (delivered_package2_l3_t6) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t6_t6
  :parameters ()
  :precondition (and (time_now_t6) (at_package2_l2))
  :effect (and (delivered_package2_l2_t6) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t6_t6
  :parameters ()
  :precondition (and (time_now_t6) (at_package2_l1))
  :effect (and (delivered_package2_l1_t6) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t6_t6
  :parameters ()
  :precondition (and (time_now_t6) (at_package1_l3))
  :effect (and (delivered_package1_l3_t6) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t6_t6
  :parameters ()
  :precondition (and (time_now_t6) (at_package1_l2))
  :effect (and (delivered_package1_l2_t6) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t6_t6
  :parameters ()
  :precondition (and (time_now_t6) (at_package1_l1))
  :effect (and (delivered_package1_l1_t6) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action drive_truck1_l1_l2_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_truck1_l1))
  :effect (and (time_now_t6) (at_truck1_l2) (not (at_truck1_l1)) (not (time_now_t5))))
 (:action drive_truck1_l1_l3_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_truck1_l1))
  :effect (and (time_now_t6) (at_truck1_l3) (not (at_truck1_l1)) (not (time_now_t5))))
 (:action drive_truck1_l2_l1_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_truck1_l2))
  :effect (and (time_now_t6) (at_truck1_l1) (not (at_truck1_l2)) (not (time_now_t5))))
 (:action drive_truck1_l2_l3_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_truck1_l2))
  :effect (and (time_now_t6) (at_truck1_l3) (not (at_truck1_l2)) (not (time_now_t5))))
 (:action drive_truck1_l3_l1_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_truck1_l3))
  :effect (and (time_now_t6) (at_truck1_l1) (not (at_truck1_l3)) (not (time_now_t5))))
 (:action drive_truck1_l3_l2_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_truck1_l3))
  :effect (and (time_now_t6) (at_truck1_l2) (not (at_truck1_l3)) (not (time_now_t5))))
 (:action deliver_package3_l3_t5_t5
  :parameters ()
  :precondition (and (time_now_t5) (at_package3_l3))
  :effect (and (delivered_package3_l3_t5) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t5_t5
  :parameters ()
  :precondition (and (time_now_t5) (at_package3_l2))
  :effect (and (delivered_package3_l2_t5) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t5_t5
  :parameters ()
  :precondition (and (time_now_t5) (at_package3_l1))
  :effect (and (delivered_package3_l1_t5) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t5_t5
  :parameters ()
  :precondition (and (time_now_t5) (at_package2_l3))
  :effect (and (delivered_package2_l3_t5) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t5_t5
  :parameters ()
  :precondition (and (time_now_t5) (at_package2_l2))
  :effect (and (delivered_package2_l2_t5) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t5_t5
  :parameters ()
  :precondition (and (time_now_t5) (at_package2_l1))
  :effect (and (delivered_package2_l1_t5) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t5_t5
  :parameters ()
  :precondition (and (time_now_t5) (at_package1_l3))
  :effect (and (delivered_package1_l3_t5) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t5_t5
  :parameters ()
  :precondition (and (time_now_t5) (at_package1_l2))
  :effect (and (delivered_package1_l2_t5) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t5_t5
  :parameters ()
  :precondition (and (time_now_t5) (at_package1_l1))
  :effect (and (delivered_package1_l1_t5) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_package3_l3))
  :effect (and (delivered_package3_l3_t6) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_package3_l2))
  :effect (and (delivered_package3_l2_t6) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_package3_l1))
  :effect (and (delivered_package3_l1_t6) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_package2_l3))
  :effect (and (delivered_package2_l3_t6) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_package2_l2))
  :effect (and (delivered_package2_l2_t6) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_package2_l1))
  :effect (and (delivered_package2_l1_t6) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_package1_l3))
  :effect (and (delivered_package1_l3_t6) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_package1_l2))
  :effect (and (delivered_package1_l2_t6) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t5_t6
  :parameters ()
  :precondition (and (time_now_t5) (at_package1_l1))
  :effect (and (delivered_package1_l1_t6) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action drive_truck1_l1_l2_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_truck1_l1))
  :effect (and (time_now_t5) (at_truck1_l2) (not (at_truck1_l1)) (not (time_now_t4))))
 (:action drive_truck1_l1_l3_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_truck1_l1))
  :effect (and (time_now_t5) (at_truck1_l3) (not (at_truck1_l1)) (not (time_now_t4))))
 (:action drive_truck1_l2_l1_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_truck1_l2))
  :effect (and (time_now_t5) (at_truck1_l1) (not (at_truck1_l2)) (not (time_now_t4))))
 (:action drive_truck1_l2_l3_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_truck1_l2))
  :effect (and (time_now_t5) (at_truck1_l3) (not (at_truck1_l2)) (not (time_now_t4))))
 (:action drive_truck1_l3_l1_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_truck1_l3))
  :effect (and (time_now_t5) (at_truck1_l1) (not (at_truck1_l3)) (not (time_now_t4))))
 (:action drive_truck1_l3_l2_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_truck1_l3))
  :effect (and (time_now_t5) (at_truck1_l2) (not (at_truck1_l3)) (not (time_now_t4))))
 (:action deliver_package3_l3_t4_t4
  :parameters ()
  :precondition (and (time_now_t4) (at_package3_l3))
  :effect (and (delivered_package3_l3_t4) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t4_t4
  :parameters ()
  :precondition (and (time_now_t4) (at_package3_l2))
  :effect (and (delivered_package3_l2_t4) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t4_t4
  :parameters ()
  :precondition (and (time_now_t4) (at_package3_l1))
  :effect (and (delivered_package3_l1_t4) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t4_t4
  :parameters ()
  :precondition (and (time_now_t4) (at_package2_l3))
  :effect (and (delivered_package2_l3_t4) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t4_t4
  :parameters ()
  :precondition (and (time_now_t4) (at_package2_l2))
  :effect (and (delivered_package2_l2_t4) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t4_t4
  :parameters ()
  :precondition (and (time_now_t4) (at_package2_l1))
  :effect (and (delivered_package2_l1_t4) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t4_t4
  :parameters ()
  :precondition (and (time_now_t4) (at_package1_l3))
  :effect (and (delivered_package1_l3_t4) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t4_t4
  :parameters ()
  :precondition (and (time_now_t4) (at_package1_l2))
  :effect (and (delivered_package1_l2_t4) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t4_t4
  :parameters ()
  :precondition (and (time_now_t4) (at_package1_l1))
  :effect (and (delivered_package1_l1_t4) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_package3_l3))
  :effect (and (delivered_package3_l3_t5) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_package3_l2))
  :effect (and (delivered_package3_l2_t5) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_package3_l1))
  :effect (and (delivered_package3_l1_t5) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_package2_l3))
  :effect (and (delivered_package2_l3_t5) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_package2_l2))
  :effect (and (delivered_package2_l2_t5) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_package2_l1))
  :effect (and (delivered_package2_l1_t5) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_package1_l3))
  :effect (and (delivered_package1_l3_t5) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_package1_l2))
  :effect (and (delivered_package1_l2_t5) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t4_t5
  :parameters ()
  :precondition (and (time_now_t4) (at_package1_l1))
  :effect (and (delivered_package1_l1_t5) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t4_t6
  :parameters ()
  :precondition (and (time_now_t4) (at_package3_l3))
  :effect (and (delivered_package3_l3_t6) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t4_t6
  :parameters ()
  :precondition (and (time_now_t4) (at_package3_l2))
  :effect (and (delivered_package3_l2_t6) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t4_t6
  :parameters ()
  :precondition (and (time_now_t4) (at_package3_l1))
  :effect (and (delivered_package3_l1_t6) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t4_t6
  :parameters ()
  :precondition (and (time_now_t4) (at_package2_l3))
  :effect (and (delivered_package2_l3_t6) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t4_t6
  :parameters ()
  :precondition (and (time_now_t4) (at_package2_l2))
  :effect (and (delivered_package2_l2_t6) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t4_t6
  :parameters ()
  :precondition (and (time_now_t4) (at_package2_l1))
  :effect (and (delivered_package2_l1_t6) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t4_t6
  :parameters ()
  :precondition (and (time_now_t4) (at_package1_l3))
  :effect (and (delivered_package1_l3_t6) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t4_t6
  :parameters ()
  :precondition (and (time_now_t4) (at_package1_l2))
  :effect (and (delivered_package1_l2_t6) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t4_t6
  :parameters ()
  :precondition (and (time_now_t4) (at_package1_l1))
  :effect (and (delivered_package1_l1_t6) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action drive_truck1_l1_l2_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_truck1_l1))
  :effect (and (time_now_t4) (at_truck1_l2) (not (at_truck1_l1)) (not (time_now_t3))))
 (:action drive_truck1_l1_l3_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_truck1_l1))
  :effect (and (time_now_t4) (at_truck1_l3) (not (at_truck1_l1)) (not (time_now_t3))))
 (:action drive_truck1_l2_l1_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_truck1_l2))
  :effect (and (time_now_t4) (at_truck1_l1) (not (at_truck1_l2)) (not (time_now_t3))))
 (:action drive_truck1_l2_l3_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_truck1_l2))
  :effect (and (time_now_t4) (at_truck1_l3) (not (at_truck1_l2)) (not (time_now_t3))))
 (:action drive_truck1_l3_l1_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_truck1_l3))
  :effect (and (time_now_t4) (at_truck1_l1) (not (at_truck1_l3)) (not (time_now_t3))))
 (:action drive_truck1_l3_l2_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_truck1_l3))
  :effect (and (time_now_t4) (at_truck1_l2) (not (at_truck1_l3)) (not (time_now_t3))))
 (:action deliver_package3_l3_t3_t3
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l3))
  :effect (and (delivered_package3_l3_t3) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t3_t3
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l2))
  :effect (and (delivered_package3_l2_t3) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t3_t3
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l1))
  :effect (and (delivered_package3_l1_t3) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t3_t3
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l3))
  :effect (and (delivered_package2_l3_t3) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t3_t3
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l2))
  :effect (and (delivered_package2_l2_t3) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t3_t3
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l1))
  :effect (and (delivered_package2_l1_t3) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t3_t3
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l3))
  :effect (and (delivered_package1_l3_t3) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t3_t3
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l2))
  :effect (and (delivered_package1_l2_t3) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t3_t3
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l1))
  :effect (and (delivered_package1_l1_t3) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l3))
  :effect (and (delivered_package3_l3_t4) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l2))
  :effect (and (delivered_package3_l2_t4) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l1))
  :effect (and (delivered_package3_l1_t4) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l3))
  :effect (and (delivered_package2_l3_t4) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l2))
  :effect (and (delivered_package2_l2_t4) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l1))
  :effect (and (delivered_package2_l1_t4) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l3))
  :effect (and (delivered_package1_l3_t4) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l2))
  :effect (and (delivered_package1_l2_t4) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t3_t4
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l1))
  :effect (and (delivered_package1_l1_t4) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t3_t5
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l3))
  :effect (and (delivered_package3_l3_t5) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t3_t5
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l2))
  :effect (and (delivered_package3_l2_t5) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t3_t5
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l1))
  :effect (and (delivered_package3_l1_t5) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t3_t5
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l3))
  :effect (and (delivered_package2_l3_t5) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t3_t5
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l2))
  :effect (and (delivered_package2_l2_t5) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t3_t5
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l1))
  :effect (and (delivered_package2_l1_t5) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t3_t5
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l3))
  :effect (and (delivered_package1_l3_t5) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t3_t5
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l2))
  :effect (and (delivered_package1_l2_t5) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t3_t5
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l1))
  :effect (and (delivered_package1_l1_t5) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t3_t6
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l3))
  :effect (and (delivered_package3_l3_t6) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t3_t6
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l2))
  :effect (and (delivered_package3_l2_t6) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t3_t6
  :parameters ()
  :precondition (and (time_now_t3) (at_package3_l1))
  :effect (and (delivered_package3_l1_t6) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t3_t6
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l3))
  :effect (and (delivered_package2_l3_t6) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t3_t6
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l2))
  :effect (and (delivered_package2_l2_t6) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t3_t6
  :parameters ()
  :precondition (and (time_now_t3) (at_package2_l1))
  :effect (and (delivered_package2_l1_t6) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t3_t6
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l3))
  :effect (and (delivered_package1_l3_t6) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t3_t6
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l2))
  :effect (and (delivered_package1_l2_t6) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t3_t6
  :parameters ()
  :precondition (and (time_now_t3) (at_package1_l1))
  :effect (and (delivered_package1_l1_t6) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action drive_truck1_l1_l2_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_truck1_l1))
  :effect (and (time_now_t3) (at_truck1_l2) (not (at_truck1_l1)) (not (time_now_t2))))
 (:action drive_truck1_l1_l3_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_truck1_l1))
  :effect (and (time_now_t3) (at_truck1_l3) (not (at_truck1_l1)) (not (time_now_t2))))
 (:action drive_truck1_l2_l1_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_truck1_l2))
  :effect (and (time_now_t3) (at_truck1_l1) (not (at_truck1_l2)) (not (time_now_t2))))
 (:action drive_truck1_l2_l3_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_truck1_l2))
  :effect (and (time_now_t3) (at_truck1_l3) (not (at_truck1_l2)) (not (time_now_t2))))
 (:action drive_truck1_l3_l1_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_truck1_l3))
  :effect (and (time_now_t3) (at_truck1_l1) (not (at_truck1_l3)) (not (time_now_t2))))
 (:action drive_truck1_l3_l2_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_truck1_l3))
  :effect (and (time_now_t3) (at_truck1_l2) (not (at_truck1_l3)) (not (time_now_t2))))
 (:action load_package3_truck1_a2_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (at_package3_l3) (free_a2_truck1) (free_a1_truck1))
  :effect (and (in_package3_truck1_a2) (not (free_a2_truck1)) (not (at_package3_l3))))
 (:action load_package3_truck1_a2_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (at_package3_l1) (free_a2_truck1) (free_a1_truck1))
  :effect (and (in_package3_truck1_a2) (not (free_a2_truck1)) (not (at_package3_l1))))
 (:action load_package3_truck1_a1_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (at_package3_l3) (free_a1_truck1))
  :effect (and (in_package3_truck1_a1) (not (free_a1_truck1)) (not (at_package3_l3))))
 (:action load_package3_truck1_a1_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (at_package3_l1) (free_a1_truck1))
  :effect (and (in_package3_truck1_a1) (not (free_a1_truck1)) (not (at_package3_l1))))
 (:action load_package2_truck1_a2_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (at_package2_l3) (free_a2_truck1) (free_a1_truck1))
  :effect (and (in_package2_truck1_a2) (not (free_a2_truck1)) (not (at_package2_l3))))
 (:action load_package2_truck1_a2_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (at_package2_l1) (free_a2_truck1) (free_a1_truck1))
  :effect (and (in_package2_truck1_a2) (not (free_a2_truck1)) (not (at_package2_l1))))
 (:action load_package2_truck1_a1_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (at_package2_l3) (free_a1_truck1))
  :effect (and (in_package2_truck1_a1) (not (free_a1_truck1)) (not (at_package2_l3))))
 (:action load_package2_truck1_a1_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (at_package2_l1) (free_a1_truck1))
  :effect (and (in_package2_truck1_a1) (not (free_a1_truck1)) (not (at_package2_l1))))
 (:action load_package1_truck1_a2_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (at_package1_l3) (free_a2_truck1) (free_a1_truck1))
  :effect (and (in_package1_truck1_a2) (not (free_a2_truck1)) (not (at_package1_l3))))
 (:action load_package1_truck1_a2_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (at_package1_l1) (free_a2_truck1) (free_a1_truck1))
  :effect (and (in_package1_truck1_a2) (not (free_a2_truck1)) (not (at_package1_l1))))
 (:action load_package1_truck1_a1_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (at_package1_l3) (free_a1_truck1))
  :effect (and (in_package1_truck1_a1) (not (free_a1_truck1)) (not (at_package1_l3))))
 (:action load_package1_truck1_a1_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (at_package1_l1) (free_a1_truck1))
  :effect (and (in_package1_truck1_a1) (not (free_a1_truck1)) (not (at_package1_l1))))
 (:action deliver_package3_l3_t1_t1
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l3))
  :effect (and (delivered_package3_l3_t1) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l1_t1_t1
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l1))
  :effect (and (delivered_package3_l1_t1) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t1_t1
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l3))
  :effect (and (delivered_package2_l3_t1) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l1_t1_t1
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l1))
  :effect (and (delivered_package2_l1_t1) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t1_t1
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l3))
  :effect (and (delivered_package1_l3_t1) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l1_t1_t1
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l1))
  :effect (and (delivered_package1_l1_t1) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l3))
  :effect (and (delivered_package3_l3_t2) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l1_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l1))
  :effect (and (delivered_package3_l1_t2) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l3))
  :effect (and (delivered_package2_l3_t2) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l1_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l1))
  :effect (and (delivered_package2_l1_t2) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l3))
  :effect (and (delivered_package1_l3_t2) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l1_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l1))
  :effect (and (delivered_package1_l1_t2) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t1_t3
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l3))
  :effect (and (delivered_package3_l3_t3) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l1_t1_t3
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l1))
  :effect (and (delivered_package3_l1_t3) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t1_t3
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l3))
  :effect (and (delivered_package2_l3_t3) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l1_t1_t3
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l1))
  :effect (and (delivered_package2_l1_t3) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t1_t3
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l3))
  :effect (and (delivered_package1_l3_t3) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l1_t1_t3
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l1))
  :effect (and (delivered_package1_l1_t3) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t1_t4
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l3))
  :effect (and (delivered_package3_l3_t4) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l1_t1_t4
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l1))
  :effect (and (delivered_package3_l1_t4) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t1_t4
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l3))
  :effect (and (delivered_package2_l3_t4) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l1_t1_t4
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l1))
  :effect (and (delivered_package2_l1_t4) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t1_t4
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l3))
  :effect (and (delivered_package1_l3_t4) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l1_t1_t4
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l1))
  :effect (and (delivered_package1_l1_t4) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t1_t5
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l3))
  :effect (and (delivered_package3_l3_t5) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l1_t1_t5
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l1))
  :effect (and (delivered_package3_l1_t5) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t1_t5
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l3))
  :effect (and (delivered_package2_l3_t5) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l1_t1_t5
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l1))
  :effect (and (delivered_package2_l1_t5) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t1_t5
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l3))
  :effect (and (delivered_package1_l3_t5) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l1_t1_t5
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l1))
  :effect (and (delivered_package1_l1_t5) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t1_t6
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l3))
  :effect (and (delivered_package3_l3_t6) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l1_t1_t6
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l1))
  :effect (and (delivered_package3_l1_t6) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t1_t6
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l3))
  :effect (and (delivered_package2_l3_t6) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l1_t1_t6
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l1))
  :effect (and (delivered_package2_l1_t6) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t1_t6
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l3))
  :effect (and (delivered_package1_l3_t6) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l1_t1_t6
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l1))
  :effect (and (delivered_package1_l1_t6) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t2_t2
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l3))
  :effect (and (delivered_package3_l3_t2) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t2_t2
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l2))
  :effect (and (delivered_package3_l2_t2) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t2_t2
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l1))
  :effect (and (delivered_package3_l1_t2) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t2_t2
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l3))
  :effect (and (delivered_package2_l3_t2) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t2_t2
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l2))
  :effect (and (delivered_package2_l2_t2) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t2_t2
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l1))
  :effect (and (delivered_package2_l1_t2) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t2_t2
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l3))
  :effect (and (delivered_package1_l3_t2) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t2_t2
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l2))
  :effect (and (delivered_package1_l2_t2) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t2_t2
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l1))
  :effect (and (delivered_package1_l1_t2) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l3))
  :effect (and (delivered_package3_l3_t3) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l2))
  :effect (and (delivered_package3_l2_t3) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l1))
  :effect (and (delivered_package3_l1_t3) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l3))
  :effect (and (delivered_package2_l3_t3) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l2))
  :effect (and (delivered_package2_l2_t3) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l1))
  :effect (and (delivered_package2_l1_t3) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l3))
  :effect (and (delivered_package1_l3_t3) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l2))
  :effect (and (delivered_package1_l2_t3) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t2_t3
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l1))
  :effect (and (delivered_package1_l1_t3) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t2_t4
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l3))
  :effect (and (delivered_package3_l3_t4) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t2_t4
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l2))
  :effect (and (delivered_package3_l2_t4) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t2_t4
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l1))
  :effect (and (delivered_package3_l1_t4) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t2_t4
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l3))
  :effect (and (delivered_package2_l3_t4) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t2_t4
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l2))
  :effect (and (delivered_package2_l2_t4) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t2_t4
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l1))
  :effect (and (delivered_package2_l1_t4) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t2_t4
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l3))
  :effect (and (delivered_package1_l3_t4) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t2_t4
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l2))
  :effect (and (delivered_package1_l2_t4) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t2_t4
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l1))
  :effect (and (delivered_package1_l1_t4) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t2_t5
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l3))
  :effect (and (delivered_package3_l3_t5) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t2_t5
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l2))
  :effect (and (delivered_package3_l2_t5) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t2_t5
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l1))
  :effect (and (delivered_package3_l1_t5) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t2_t5
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l3))
  :effect (and (delivered_package2_l3_t5) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t2_t5
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l2))
  :effect (and (delivered_package2_l2_t5) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t2_t5
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l1))
  :effect (and (delivered_package2_l1_t5) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t2_t5
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l3))
  :effect (and (delivered_package1_l3_t5) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t2_t5
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l2))
  :effect (and (delivered_package1_l2_t5) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t2_t5
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l1))
  :effect (and (delivered_package1_l1_t5) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action deliver_package3_l3_t2_t6
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l3))
  :effect (and (delivered_package3_l3_t6) (at_destination_package3_l3) (not (at_package3_l3))))
 (:action deliver_package3_l2_t2_t6
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l2))
  :effect (and (delivered_package3_l2_t6) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package3_l1_t2_t6
  :parameters ()
  :precondition (and (time_now_t2) (at_package3_l1))
  :effect (and (delivered_package3_l1_t6) (at_destination_package3_l1) (not (at_package3_l1))))
 (:action deliver_package2_l3_t2_t6
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l3))
  :effect (and (delivered_package2_l3_t6) (at_destination_package2_l3) (not (at_package2_l3))))
 (:action deliver_package2_l2_t2_t6
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l2))
  :effect (and (delivered_package2_l2_t6) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package2_l1_t2_t6
  :parameters ()
  :precondition (and (time_now_t2) (at_package2_l1))
  :effect (and (delivered_package2_l1_t6) (at_destination_package2_l1) (not (at_package2_l1))))
 (:action deliver_package1_l3_t2_t6
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l3))
  :effect (and (delivered_package1_l3_t6) (at_destination_package1_l3) (not (at_package1_l3))))
 (:action deliver_package1_l2_t2_t6
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l2))
  :effect (and (delivered_package1_l2_t6) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package1_l1_t2_t6
  :parameters ()
  :precondition (and (time_now_t2) (at_package1_l1))
  :effect (and (delivered_package1_l1_t6) (at_destination_package1_l1) (not (at_package1_l1))))
 (:action drive_truck1_l1_l2_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_truck1_l1))
  :effect (and (time_now_t2) (at_truck1_l2) (not (at_truck1_l1)) (not (time_now_t1))))
 (:action drive_truck1_l1_l3_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_truck1_l1))
  :effect (and (time_now_t2) (at_truck1_l3) (not (at_truck1_l1)) (not (time_now_t1))))
 (:action drive_truck1_l2_l1_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_truck1_l2))
  :effect (and (time_now_t2) (at_truck1_l1) (not (at_truck1_l2)) (not (time_now_t1))))
 (:action drive_truck1_l2_l3_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_truck1_l2))
  :effect (and (time_now_t2) (at_truck1_l3) (not (at_truck1_l2)) (not (time_now_t1))))
 (:action drive_truck1_l3_l1_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_truck1_l3))
  :effect (and (time_now_t2) (at_truck1_l1) (not (at_truck1_l3)) (not (time_now_t1))))
 (:action drive_truck1_l3_l2_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_truck1_l3))
  :effect (and (time_now_t2) (at_truck1_l2) (not (at_truck1_l3)) (not (time_now_t1))))
 (:action unload_package3_truck1_a2_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (in_package3_truck1_a2) (free_a1_truck1))
  :effect (and (at_package3_l3) (free_a2_truck1) (not (in_package3_truck1_a2))))
 (:action unload_package3_truck1_a2_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (in_package3_truck1_a2) (free_a1_truck1))
  :effect (and (at_package3_l2) (free_a2_truck1) (not (in_package3_truck1_a2))))
 (:action unload_package3_truck1_a2_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (in_package3_truck1_a2) (free_a1_truck1))
  :effect (and (at_package3_l1) (free_a2_truck1) (not (in_package3_truck1_a2))))
 (:action unload_package3_truck1_a1_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (in_package3_truck1_a1))
  :effect (and (at_package3_l3) (free_a1_truck1) (not (in_package3_truck1_a1))))
 (:action unload_package3_truck1_a1_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (in_package3_truck1_a1))
  :effect (and (at_package3_l2) (free_a1_truck1) (not (in_package3_truck1_a1))))
 (:action unload_package3_truck1_a1_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (in_package3_truck1_a1))
  :effect (and (at_package3_l1) (free_a1_truck1) (not (in_package3_truck1_a1))))
 (:action unload_package2_truck1_a2_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (in_package2_truck1_a2) (free_a1_truck1))
  :effect (and (at_package2_l3) (free_a2_truck1) (not (in_package2_truck1_a2))))
 (:action unload_package2_truck1_a2_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (in_package2_truck1_a2) (free_a1_truck1))
  :effect (and (at_package2_l2) (free_a2_truck1) (not (in_package2_truck1_a2))))
 (:action unload_package2_truck1_a2_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (in_package2_truck1_a2) (free_a1_truck1))
  :effect (and (at_package2_l1) (free_a2_truck1) (not (in_package2_truck1_a2))))
 (:action unload_package2_truck1_a1_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (in_package2_truck1_a1))
  :effect (and (at_package2_l3) (free_a1_truck1) (not (in_package2_truck1_a1))))
 (:action unload_package2_truck1_a1_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (in_package2_truck1_a1))
  :effect (and (at_package2_l2) (free_a1_truck1) (not (in_package2_truck1_a1))))
 (:action unload_package2_truck1_a1_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (in_package2_truck1_a1))
  :effect (and (at_package2_l1) (free_a1_truck1) (not (in_package2_truck1_a1))))
 (:action unload_package1_truck1_a2_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (in_package1_truck1_a2) (free_a1_truck1))
  :effect (and (at_package1_l3) (free_a2_truck1) (not (in_package1_truck1_a2))))
 (:action unload_package1_truck1_a2_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (in_package1_truck1_a2) (free_a1_truck1))
  :effect (and (at_package1_l2) (free_a2_truck1) (not (in_package1_truck1_a2))))
 (:action unload_package1_truck1_a2_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (in_package1_truck1_a2) (free_a1_truck1))
  :effect (and (at_package1_l1) (free_a2_truck1) (not (in_package1_truck1_a2))))
 (:action unload_package1_truck1_a1_l3
  :parameters ()
  :precondition (and (at_truck1_l3) (in_package1_truck1_a1))
  :effect (and (at_package1_l3) (free_a1_truck1) (not (in_package1_truck1_a1))))
 (:action unload_package1_truck1_a1_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (in_package1_truck1_a1))
  :effect (and (at_package1_l2) (free_a1_truck1) (not (in_package1_truck1_a1))))
 (:action unload_package1_truck1_a1_l1
  :parameters ()
  :precondition (and (at_truck1_l1) (in_package1_truck1_a1))
  :effect (and (at_package1_l1) (free_a1_truck1) (not (in_package1_truck1_a1))))
 (:action load_package3_truck1_a2_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (at_package3_l2) (free_a2_truck1) (free_a1_truck1))
  :effect (and (in_package3_truck1_a2) (not (free_a2_truck1)) (not (at_package3_l2))))
 (:action load_package3_truck1_a1_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (at_package3_l2) (free_a1_truck1))
  :effect (and (in_package3_truck1_a1) (not (free_a1_truck1)) (not (at_package3_l2))))
 (:action load_package2_truck1_a2_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (at_package2_l2) (free_a2_truck1) (free_a1_truck1))
  :effect (and (in_package2_truck1_a2) (not (free_a2_truck1)) (not (at_package2_l2))))
 (:action load_package2_truck1_a1_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (at_package2_l2) (free_a1_truck1))
  :effect (and (in_package2_truck1_a1) (not (free_a1_truck1)) (not (at_package2_l2))))
 (:action load_package1_truck1_a2_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (at_package1_l2) (free_a2_truck1) (free_a1_truck1))
  :effect (and (in_package1_truck1_a2) (not (free_a2_truck1)) (not (at_package1_l2))))
 (:action load_package1_truck1_a1_l2
  :parameters ()
  :precondition (and (at_truck1_l2) (at_package1_l2) (free_a1_truck1))
  :effect (and (in_package1_truck1_a1) (not (free_a1_truck1)) (not (at_package1_l2))))
 (:action deliver_package3_l2_t1_t1
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l2))
  :effect (and (delivered_package3_l2_t1) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package2_l2_t1_t1
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l2))
  :effect (and (delivered_package2_l2_t1) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package1_l2_t1_t1
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l2))
  :effect (and (delivered_package1_l2_t1) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package3_l2_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l2))
  :effect (and (delivered_package3_l2_t2) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package2_l2_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l2))
  :effect (and (delivered_package2_l2_t2) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package1_l2_t1_t2
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l2))
  :effect (and (delivered_package1_l2_t2) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package3_l2_t1_t3
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l2))
  :effect (and (delivered_package3_l2_t3) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package2_l2_t1_t3
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l2))
  :effect (and (delivered_package2_l2_t3) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package1_l2_t1_t3
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l2))
  :effect (and (delivered_package1_l2_t3) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package3_l2_t1_t4
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l2))
  :effect (and (delivered_package3_l2_t4) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package2_l2_t1_t4
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l2))
  :effect (and (delivered_package2_l2_t4) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package1_l2_t1_t4
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l2))
  :effect (and (delivered_package1_l2_t4) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package3_l2_t1_t5
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l2))
  :effect (and (delivered_package3_l2_t5) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package2_l2_t1_t5
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l2))
  :effect (and (delivered_package2_l2_t5) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package1_l2_t1_t5
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l2))
  :effect (and (delivered_package1_l2_t5) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action deliver_package3_l2_t1_t6
  :parameters ()
  :precondition (and (time_now_t1) (at_package3_l2))
  :effect (and (delivered_package3_l2_t6) (at_destination_package3_l2) (not (at_package3_l2))))
 (:action deliver_package2_l2_t1_t6
  :parameters ()
  :precondition (and (time_now_t1) (at_package2_l2))
  :effect (and (delivered_package2_l2_t6) (at_destination_package2_l2) (not (at_package2_l2))))
 (:action deliver_package1_l2_t1_t6
  :parameters ()
  :precondition (and (time_now_t1) (at_package1_l2))
  :effect (and (delivered_package1_l2_t6) (at_destination_package1_l2) (not (at_package1_l2))))
 (:action drive_truck1_l1_l2_t0_t1
  :parameters ()
  :precondition (and (time_now_t0) (at_truck1_l1))
  :effect (and (time_now_t1) (at_truck1_l2) (not (at_truck1_l1)) (not (time_now_t0))))
 (:action drive_truck1_l1_l3_t0_t1
  :parameters ()
  :precondition (and (time_now_t0) (at_truck1_l1))
  :effect (and (time_now_t1) (at_truck1_l3) (not (at_truck1_l1)) (not (time_now_t0))))
 (:action drive_truck1_l2_l1_t0_t1
  :parameters ()
  :precondition (and (time_now_t0) (at_truck1_l2))
  :effect (and (time_now_t1) (at_truck1_l1) (not (at_truck1_l2)) (not (time_now_t0))))
 (:action drive_truck1_l2_l3_t0_t1
  :parameters ()
  :precondition (and (time_now_t0) (at_truck1_l2))
  :effect (and (time_now_t1) (at_truck1_l3) (not (at_truck1_l2)) (not (time_now_t0))))
 (:action drive_truck1_l3_l1_t0_t1
  :parameters ()
  :precondition (and (time_now_t0) (at_truck1_l3))
  :effect (and (time_now_t1) (at_truck1_l1) (not (at_truck1_l3)) (not (time_now_t0))))
 (:action drive_truck1_l3_l2_t0_t1
  :parameters ()
  :precondition (and (time_now_t0) (at_truck1_l3))
  :effect (and (time_now_t1) (at_truck1_l2) (not (at_truck1_l3)) (not (time_now_t0))))
)
