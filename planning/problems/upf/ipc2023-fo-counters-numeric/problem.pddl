(define (problem instance_2-problem)
 (:domain instance_2-domain)
 (:objects
   c0 c1 - counter
 )
 (:init (= (max_int) 4) (= (value c0) 0) (= (value c1) 0) (= (rate_value c0) 0) (= (rate_value c1) 0) (= (total-cost) 0))
 (:goal (and (<= (+ 1 (value c0)) (value c1))))
 (:metric minimize (total-cost))
)
