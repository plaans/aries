(define (problem mixed_f2_p1_u0_v0_g0_a0_n0_a0_b0_n0_f0_r0-problem)
 (:domain mixed_f2_p1_u0_v0_g0_a0_n0_a0_b0_n0_f0_r0-domain)
 (:objects
   p0 - passenger
   f0 f1 - floor
 )
 (:init (above f0 f1) (origin p0 f1) (destin p0 f0) (lift_at f0))
 (:goal (and (served p0)))
)
