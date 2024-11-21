(define (problem network1new_all_6_2_instance-problem)
 (:domain network1new_all_6_2_instance-domain)
 (:objects
   s12 s13 - pipe
   a1 a2 a3 - area
   lco gasoleo rat_a oca1 oc1b - product
   b0 b3 b1 b4 b2 b5 - batch_atom
 )
 (:init (normal s12) (normal s13) (may_interface lco lco) (may_interface gasoleo gasoleo) (may_interface rat_a rat_a) (may_interface oca1 oca1) (may_interface oc1b oc1b) (may_interface lco gasoleo) (may_interface gasoleo lco) (may_interface lco oca1) (may_interface oca1 lco) (may_interface lco oc1b) (may_interface oc1b lco) (may_interface lco rat_a) (may_interface rat_a lco) (may_interface gasoleo rat_a) (may_interface rat_a gasoleo) (may_interface gasoleo oca1) (may_interface oca1 gasoleo) (may_interface gasoleo oc1b) (may_interface oc1b gasoleo) (may_interface oca1 oc1b) (may_interface oc1b oca1) (connect a1 a2 s12) (connect a1 a3 s13) (is_product b0 oc1b) (is_product b3 rat_a) (is_product b1 lco) (is_product b4 lco) (is_product b2 gasoleo) (is_product b5 oca1) (on b0 a1) (on b3 a1) (on b1 a3) (on b2 a1) (first b4 s12) (last b4 s12) (first b5 s13) (last b5 s13) (unitary s12) (unitary s13))
 (:goal (and (on b2 a3) (on b5 a2)))
)
