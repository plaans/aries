(define (problem depotprob7512-problem)
 (:domain depotprob7512-domain)
 (:objects
   depot0 - depot
   distributor0 distributor1 - distributor
   truck0 truck1 - truck
   hoist0 hoist1 hoist2 - hoist
   pallet0 pallet1 pallet2 - pallet
   crate0 crate1 crate2 crate3 - crate
 )
 (:init (at_ pallet0 depot0) (clear crate0) (at_ pallet1 distributor0) (clear crate3) (at_ pallet2 distributor1) (clear crate2) (at_ truck0 depot0) (= (current_load truck0) 0) (= (load_limit truck0) 411) (at_ truck1 depot0) (= (current_load truck1) 0) (= (load_limit truck1) 390) (at_ hoist0 depot0) (available hoist0) (at_ hoist1 distributor0) (available hoist1) (at_ hoist2 distributor1) (available hoist2) (at_ crate0 depot0) (on crate0 pallet0) (= (weight crate0) 32) (at_ crate1 distributor1) (on crate1 pallet2) (= (weight crate1) 4) (at_ crate2 distributor1) (on crate2 crate1) (= (weight crate2) 89) (at_ crate3 distributor0) (on crate3 pallet1) (= (weight crate3) 62) (= (fuel_cost) 0))
 (:goal (and (on crate0 pallet2) (on crate1 crate3) (on crate2 pallet0) (on crate3 pallet1)))
 (:metric minimize (fuel_cost))
)
