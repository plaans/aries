(define (problem scanalyzer3d_52-problem)
 (:domain scanalyzer3d_52-domain)
 (:objects
   seg_in_1a seg_in_1b seg_out_1a seg_out_1b - segment
   car_in_1a car_in_1b car_out_1a car_out_1b - car
 )
 (:init (cycle_4 seg_in_1a seg_in_1b seg_out_1a seg_out_1b) (cycle_4_with_analysis seg_in_1a seg_in_1b seg_out_1a seg_out_1b) (on car_in_1a seg_in_1a) (on car_in_1b seg_in_1b) (on car_out_1a seg_out_1a) (on car_out_1b seg_out_1b) (= (total-cost) 0))
 (:goal (and (analyzed car_in_1a) (analyzed car_in_1b) (analyzed car_out_1a) (analyzed car_out_1b) (on car_in_1a seg_out_1b) (on car_in_1b seg_in_1a) (on car_out_1a seg_in_1b) (on car_out_1b seg_out_1a)))
 (:metric minimize (total-cost))
)
