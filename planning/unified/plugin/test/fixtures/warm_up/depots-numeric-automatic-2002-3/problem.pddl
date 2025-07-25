(define (problem depotprob1935-problem)
 (:domain depotprob1935-domain)
 (:objects
   depot0 - depot
   distributor0 distributor1 - distributor
   truck0 truck1 - truck
   hoist0 hoist1 hoist2 - hoist
   pallet0 pallet1 pallet2 - pallet
   crate0 crate1 crate2 crate3 crate4 crate5 - crate
 )
 (:init (at_ pallet0 depot0) (clear crate1) (at_ pallet1 distributor0) (clear crate4) (at_ pallet2 distributor1) (clear crate5) (at_ truck0 depot0) (= (current_load truck0) 0) (= (load_limit truck0) 457) (at_ truck1 distributor0) (= (current_load truck1) 0) (= (load_limit truck1) 331) (at_ hoist0 depot0) (available hoist0) (at_ hoist1 distributor0) (available hoist1) (at_ hoist2 distributor1) (available hoist2) (at_ crate0 distributor0) (on crate0 pallet1) (= (weight crate0) 99) (at_ crate1 depot0) (on crate1 pallet0) (= (weight crate1) 89) (at_ crate2 distributor1) (on crate2 pallet2) (= (weight crate2) 67) (at_ crate3 distributor0) (on crate3 crate0) (= (weight crate3) 81) (at_ crate4 distributor0) (on crate4 crate3) (= (weight crate4) 4) (at_ crate5 distributor1) (on crate5 crate2) (= (weight crate5) 50) (= (fuel_cost) 0))
 (:goal (and (on crate0 crate1) (on crate1 pallet2) (on crate2 pallet0) (on crate3 crate2) (on crate4 pallet1) (on crate5 crate0)))
 (:metric minimize (total-time))
)
