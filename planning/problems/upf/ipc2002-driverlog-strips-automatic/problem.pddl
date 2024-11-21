(define (problem dlog_2_2_2-problem)
 (:domain dlog_2_2_2-domain)
 (:objects
   s0 s1 s2 p1_0 p1_2 - location
   driver1 driver2 - driver
   truck1 truck2 - truck
   package1 package2 - obj
 )
 (:init (at_ driver1 s2) (at_ driver2 s2) (at_ truck1 s0) (empty truck1) (at_ truck2 s0) (empty truck2) (at_ package1 s0) (at_ package2 s0) (path s1 p1_0) (path p1_0 s1) (path s0 p1_0) (path p1_0 s0) (path s1 p1_2) (path p1_2 s1) (path s2 p1_2) (path p1_2 s2) (link s0 s1) (link s1 s0) (link s0 s2) (link s2 s0) (link s2 s1) (link s1 s2))
 (:goal (and (at_ driver1 s1) (at_ truck1 s1) (at_ package1 s0) (at_ package2 s0)))
)
