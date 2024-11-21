(define (problem os_sequencedstrips_p5_1-problem)
 (:domain os_sequencedstrips_p5_1-domain)
 (:objects
   n0 n1 n2 n3 n4 n5 - count
 )
 (:init (next_count n0 n1) (next_count n1 n2) (next_count n2 n3) (next_count n3 n4) (next_count n4 n5) (stacks_avail n0) (waiting o1) (includes o1 p2) (waiting o2) (includes o2 p1) (includes o2 p2) (waiting o3) (includes o3 p3) (waiting o4) (includes o4 p3) (includes o4 p4) (waiting o5) (includes o5 p5) (not_made p1) (not_made p2) (not_made p3) (not_made p4) (not_made p5) (= (total-cost) 0))
 (:goal (and (shipped o1) (shipped o2) (shipped o3) (shipped o4) (shipped o5)))
 (:metric minimize (total-cost))
)
