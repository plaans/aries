(define (problem wood_prob-problem)
 (:domain wood_prob-domain)
 (:objects
   red black - acolour
   pine teak - awood
   rough - surface
   colourfragments - treatmentstatus
   s0 s1 s2 s3 - aboardsize
   highspeed_saw0 - highspeed_saw
   glazer0 - glazer
   grinder0 - grinder
   immersion_varnisher0 - immersion_varnisher
   planer0 - planer
   saw0 - saw
   spray_varnisher0 - spray_varnisher
   b0 - board
   p0 p1 p2 - part
 )
 (:init (grind_treatment_change varnished colourfragments) (grind_treatment_change glazed untreated) (grind_treatment_change untreated untreated) (grind_treatment_change colourfragments untreated) (is_smooth smooth) (is_smooth verysmooth) (= (total_cost) 0) (boardsize_successor s0 s1) (boardsize_successor s1 s2) (boardsize_successor s2 s3) (has_colour glazer0 natural) (has_colour glazer0 red) (has_colour immersion_varnisher0 natural) (has_colour immersion_varnisher0 red) (empty highspeed_saw0) (has_colour spray_varnisher0 natural) (has_colour spray_varnisher0 red) (available p0) (colour p0 red) (wood p0 pine) (surface_condition p0 smooth) (treatment p0 varnished) (goalsize p0 small) (= (spray_varnish_cost p0) 5) (= (glaze_cost p0) 10) (= (grind_cost p0) 15) (= (plane_cost p0) 10) (unused p1) (goalsize p1 medium) (= (spray_varnish_cost p1) 10) (= (glaze_cost p1) 15) (= (grind_cost p1) 30) (= (plane_cost p1) 20) (available p2) (colour p2 natural) (wood p2 teak) (surface_condition p2 verysmooth) (treatment p2 varnished) (goalsize p2 large) (= (spray_varnish_cost p2) 15) (= (glaze_cost p2) 20) (= (grind_cost p2) 45) (= (plane_cost p2) 30) (boardsize b0 s3) (wood b0 pine) (surface_condition b0 rough) (available b0))
 (:goal (and (available p0) (colour p0 natural) (wood p0 pine) (available p1) (colour p1 natural) (wood p1 pine) (surface_condition p1 smooth) (treatment p1 varnished) (available p2) (colour p2 red) (wood p2 teak)))
 (:metric minimize (total_cost))
)
