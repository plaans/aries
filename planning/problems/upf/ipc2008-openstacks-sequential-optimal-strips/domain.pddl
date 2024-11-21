(define (domain os_sequencedstrips_p5_1-domain)
 (:requirements :strips :typing :action-costs)
 (:types order product count)
 (:constants
   p3 p5 p2 p1 p4 - product
   o5 o1 o2 o4 o3 - order
 )
 (:predicates (includes ?o - order ?p - product) (waiting ?o - order) (started ?o - order) (shipped ?o - order) (made ?p - product) (not_made ?p - product) (stacks_avail ?s - count) (next_count ?s - count ?ns - count))
 (:functions (total-cost))
 (:action open_new_stack
  :parameters ( ?open - count ?new_open - count)
  :precondition (and (stacks_avail ?open) (next_count ?open ?new_open))
  :effect (and (not (stacks_avail ?open)) (stacks_avail ?new_open) (increase (total-cost) 1)))
 (:action start_order
  :parameters ( ?o - order ?avail - count ?new_avail - count)
  :precondition (and (waiting ?o) (stacks_avail ?avail) (next_count ?new_avail ?avail))
  :effect (and (not (waiting ?o)) (started ?o) (not (stacks_avail ?avail)) (stacks_avail ?new_avail) (increase (total-cost) 0)))
 (:action make_product_p1
  :parameters ()
  :precondition (and (not_made p1) (started o2))
  :effect (and (not (not_made p1)) (made p1) (increase (total-cost) 0)))
 (:action make_product_p2
  :parameters ()
  :precondition (and (not_made p2) (started o1) (started o2))
  :effect (and (not (not_made p2)) (made p2) (increase (total-cost) 0)))
 (:action make_product_p3
  :parameters ()
  :precondition (and (not_made p3) (started o3) (started o4))
  :effect (and (not (not_made p3)) (made p3) (increase (total-cost) 0)))
 (:action make_product_p4
  :parameters ()
  :precondition (and (not_made p4) (started o4))
  :effect (and (not (not_made p4)) (made p4) (increase (total-cost) 0)))
 (:action make_product_p5
  :parameters ()
  :precondition (and (not_made p5) (started o5))
  :effect (and (not (not_made p5)) (made p5) (increase (total-cost) 0)))
 (:action ship_order_o1
  :parameters ( ?avail - count ?new_avail - count)
  :precondition (and (started o1) (made p2) (stacks_avail ?avail) (next_count ?avail ?new_avail))
  :effect (and (not (started o1)) (shipped o1) (not (stacks_avail ?avail)) (stacks_avail ?new_avail) (increase (total-cost) 0)))
 (:action ship_order_o2
  :parameters ( ?avail - count ?new_avail - count)
  :precondition (and (started o2) (made p1) (made p2) (stacks_avail ?avail) (next_count ?avail ?new_avail))
  :effect (and (not (started o2)) (shipped o2) (not (stacks_avail ?avail)) (stacks_avail ?new_avail) (increase (total-cost) 0)))
 (:action ship_order_o3
  :parameters ( ?avail - count ?new_avail - count)
  :precondition (and (started o3) (made p3) (stacks_avail ?avail) (next_count ?avail ?new_avail))
  :effect (and (not (started o3)) (shipped o3) (not (stacks_avail ?avail)) (stacks_avail ?new_avail) (increase (total-cost) 0)))
 (:action ship_order_o4
  :parameters ( ?avail - count ?new_avail - count)
  :precondition (and (started o4) (made p3) (made p4) (stacks_avail ?avail) (next_count ?avail ?new_avail))
  :effect (and (not (started o4)) (shipped o4) (not (stacks_avail ?avail)) (stacks_avail ?new_avail) (increase (total-cost) 0)))
 (:action ship_order_o5
  :parameters ( ?avail - count ?new_avail - count)
  :precondition (and (started o5) (made p5) (stacks_avail ?avail) (next_count ?avail ?new_avail))
  :effect (and (not (started o5)) (shipped o5) (not (stacks_avail ?avail)) (stacks_avail ?new_avail) (increase (total-cost) 0)))
)
