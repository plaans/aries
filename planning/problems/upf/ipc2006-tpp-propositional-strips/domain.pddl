(define (domain grounded_strips_tpp-domain)
 (:requirements :strips)
 (:predicates (at_truck1_market1) (on_sale_goods1_market1_level0) (ready_to_load_goods1_market1_level1) (loaded_goods1_truck1_level1) (stored_goods1_level1) (stored_goods1_level0) (loaded_goods1_truck1_level0) (on_sale_goods1_market1_level1) (ready_to_load_goods1_market1_level0) (at_truck1_depot1))
 (:action unload_goods1_truck1_depot1_level0_level1_level0_level1
  :parameters ()
  :precondition (and (stored_goods1_level0) (loaded_goods1_truck1_level1) (at_truck1_depot1))
  :effect (and (loaded_goods1_truck1_level0) (stored_goods1_level1) (not (loaded_goods1_truck1_level1)) (not (stored_goods1_level0))))
 (:action load_goods1_truck1_market1_level0_level1_level0_level1
  :parameters ()
  :precondition (and (ready_to_load_goods1_market1_level1) (loaded_goods1_truck1_level0) (at_truck1_market1))
  :effect (and (loaded_goods1_truck1_level1) (ready_to_load_goods1_market1_level0) (not (loaded_goods1_truck1_level0)) (not (ready_to_load_goods1_market1_level1))))
 (:action drive_truck1_market1_depot1
  :parameters ()
  :precondition (and (at_truck1_market1))
  :effect (and (at_truck1_depot1) (not (at_truck1_market1))))
 (:action buy_truck1_goods1_market1_level0_level1_level0_level1
  :parameters ()
  :precondition (and (ready_to_load_goods1_market1_level0) (on_sale_goods1_market1_level1) (at_truck1_market1))
  :effect (and (on_sale_goods1_market1_level0) (ready_to_load_goods1_market1_level1) (not (on_sale_goods1_market1_level1)) (not (ready_to_load_goods1_market1_level0))))
 (:action drive_truck1_depot1_market1
  :parameters ()
  :precondition (and (at_truck1_depot1))
  :effect (and (at_truck1_market1) (not (at_truck1_depot1))))
)
