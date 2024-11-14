(define (problem tpp-problem)
 (:domain tpp-domain)
 (:objects
   level0 level1 - level
   depot1 - depot
   market1 - market
   truck1 - truck
   goods1 - goods
 )
 (:init (next level1 level0) (ready_to_load goods1 market1 level0) (stored goods1 level0) (loaded goods1 truck1 level0) (connected depot1 market1) (connected market1 depot1) (on_sale goods1 market1 level1) (at_ truck1 depot1))
 (:goal (and (stored goods1 level1)))
)
