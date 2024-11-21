(define (domain wood_prob-domain)
 (:requirements :strips :typing :numeric-fluents)
 (:types
    acolour awood woodobj machine surface treatmentstatus aboardsize apartsize - object
    highspeed_saw glazer grinder immersion_varnisher planer saw spray_varnisher - machine
    board part - woodobj
 )
 (:constants
   glazed untreated varnished - treatmentstatus
   natural - acolour
   verysmooth smooth - surface
   medium large small - apartsize
 )
 (:predicates (unused ?obj - part) (available ?obj_0 - woodobj) (surface_condition ?obj_0 - woodobj ?surface - surface) (treatment ?obj - part ?treatment - treatmentstatus) (colour ?obj - part ?colour - acolour) (wood ?obj_0 - woodobj ?wood - awood) (boardsize ?board - board ?size - aboardsize) (goalsize ?part - part ?size_0 - apartsize) (boardsize_successor ?size1 - aboardsize ?size2 - aboardsize) (in_highspeed_saw ?b - board ?m - highspeed_saw) (empty ?m - highspeed_saw) (has_colour ?machine - machine ?colour - acolour) (contains_part ?b - board ?p - part) (grind_treatment_change ?old - treatmentstatus ?new - treatmentstatus) (is_smooth ?surface - surface))
 (:functions (total_cost) (spray_varnish_cost ?obj - part) (glaze_cost ?obj - part) (grind_cost ?obj - part) (plane_cost ?obj - part))
 (:action do_immersion_varnish
  :parameters ( ?x - part ?m_0 - immersion_varnisher ?newcolour - acolour ?surface - surface)
  :precondition (and (available ?x) (has_colour ?m_0 ?newcolour) (surface_condition ?x ?surface) (is_smooth ?surface) (treatment ?x untreated))
  :effect (and (increase (total_cost) 10) (not (treatment ?x untreated)) (treatment ?x varnished) (not (colour ?x natural)) (colour ?x ?newcolour)))
 (:action do_spray_varnish
  :parameters ( ?x - part ?m_1 - spray_varnisher ?newcolour - acolour ?surface - surface)
  :precondition (and (available ?x) (has_colour ?m_1 ?newcolour) (surface_condition ?x ?surface) (is_smooth ?surface) (treatment ?x untreated))
  :effect (and (increase (total_cost) (spray_varnish_cost ?x)) (not (treatment ?x untreated)) (treatment ?x varnished) (not (colour ?x natural)) (colour ?x ?newcolour)))
 (:action do_glaze
  :parameters ( ?x - part ?m_2 - glazer ?newcolour - acolour)
  :precondition (and (available ?x) (has_colour ?m_2 ?newcolour) (treatment ?x untreated))
  :effect (and (increase (total_cost) (glaze_cost ?x)) (not (treatment ?x untreated)) (treatment ?x glazed) (not (colour ?x natural)) (colour ?x ?newcolour)))
 (:action do_grind
  :parameters ( ?x - part ?m_3 - grinder ?oldsurface - surface ?oldcolour - acolour ?oldtreatment - treatmentstatus ?newtreatment - treatmentstatus)
  :precondition (and (available ?x) (surface_condition ?x ?oldsurface) (is_smooth ?oldsurface) (colour ?x ?oldcolour) (treatment ?x ?oldtreatment) (grind_treatment_change ?oldtreatment ?newtreatment))
  :effect (and (increase (total_cost) (grind_cost ?x)) (not (surface_condition ?x ?oldsurface)) (surface_condition ?x verysmooth) (not (treatment ?x ?oldtreatment)) (treatment ?x ?newtreatment) (not (colour ?x ?oldcolour)) (colour ?x natural)))
 (:action do_plane
  :parameters ( ?x - part ?m_4 - planer ?oldsurface - surface ?oldcolour - acolour ?oldtreatment - treatmentstatus)
  :precondition (and (available ?x) (surface_condition ?x ?oldsurface) (treatment ?x ?oldtreatment) (colour ?x ?oldcolour))
  :effect (and (increase (total_cost) (plane_cost ?x)) (not (surface_condition ?x ?oldsurface)) (surface_condition ?x smooth) (not (treatment ?x ?oldtreatment)) (treatment ?x untreated) (not (colour ?x ?oldcolour)) (colour ?x natural)))
 (:action load_highspeed_saw
  :parameters ( ?b - board ?m - highspeed_saw)
  :precondition (and (empty ?m) (available ?b))
  :effect (and (increase (total_cost) 30) (not (available ?b)) (not (empty ?m)) (in_highspeed_saw ?b ?m)))
 (:action unload_highspeed_saw
  :parameters ( ?b - board ?m - highspeed_saw)
  :precondition (and (in_highspeed_saw ?b ?m))
  :effect (and (increase (total_cost) 10) (available ?b) (not (in_highspeed_saw ?b ?m)) (empty ?m)))
 (:action cut_board_small
  :parameters ( ?b - board ?p - part ?m - highspeed_saw ?w - awood ?surface - surface ?size_before - aboardsize ?size_after - aboardsize)
  :precondition (and (unused ?p) (goalsize ?p small) (in_highspeed_saw ?b ?m) (wood ?b ?w) (surface_condition ?b ?surface) (boardsize ?b ?size_before) (boardsize_successor ?size_after ?size_before))
  :effect (and (increase (total_cost) 10) (not (unused ?p)) (available ?p) (wood ?p ?w) (surface_condition ?p ?surface) (colour ?p natural) (treatment ?p untreated) (boardsize ?b ?size_after)))
 (:action cut_board_medium
  :parameters ( ?b - board ?p - part ?m - highspeed_saw ?w - awood ?surface - surface ?size_before - aboardsize ?s1 - aboardsize ?size_after - aboardsize)
  :precondition (and (unused ?p) (goalsize ?p medium) (in_highspeed_saw ?b ?m) (wood ?b ?w) (surface_condition ?b ?surface) (boardsize ?b ?size_before) (boardsize_successor ?size_after ?s1) (boardsize_successor ?s1 ?size_before))
  :effect (and (increase (total_cost) 10) (not (unused ?p)) (available ?p) (wood ?p ?w) (surface_condition ?p ?surface) (colour ?p natural) (treatment ?p untreated) (boardsize ?b ?size_after)))
 (:action cut_board_large
  :parameters ( ?b - board ?p - part ?m - highspeed_saw ?w - awood ?surface - surface ?size_before - aboardsize ?s1 - aboardsize ?s2 - aboardsize ?size_after - aboardsize)
  :precondition (and (unused ?p) (goalsize ?p large) (in_highspeed_saw ?b ?m) (wood ?b ?w) (surface_condition ?b ?surface) (boardsize ?b ?size_before) (boardsize_successor ?size_after ?s1) (boardsize_successor ?s1 ?s2) (boardsize_successor ?s2 ?size_before))
  :effect (and (increase (total_cost) 10) (not (unused ?p)) (available ?p) (wood ?p ?w) (surface_condition ?p ?surface) (colour ?p natural) (treatment ?p untreated) (boardsize ?b ?size_after)))
 (:action do_saw_small
  :parameters ( ?b - board ?p - part ?m_5 - saw ?w - awood ?surface - surface ?size_before - aboardsize ?size_after - aboardsize)
  :precondition (and (unused ?p) (goalsize ?p small) (available ?b) (wood ?b ?w) (surface_condition ?b ?surface) (boardsize ?b ?size_before) (boardsize_successor ?size_after ?size_before))
  :effect (and (increase (total_cost) 30) (not (unused ?p)) (available ?p) (wood ?p ?w) (surface_condition ?p ?surface) (colour ?p natural) (treatment ?p untreated) (boardsize ?b ?size_after)))
 (:action do_saw_medium
  :parameters ( ?b - board ?p - part ?m_5 - saw ?w - awood ?surface - surface ?size_before - aboardsize ?s1 - aboardsize ?size_after - aboardsize)
  :precondition (and (unused ?p) (goalsize ?p medium) (available ?b) (wood ?b ?w) (surface_condition ?b ?surface) (boardsize ?b ?size_before) (boardsize_successor ?size_after ?s1) (boardsize_successor ?s1 ?size_before))
  :effect (and (increase (total_cost) 30) (not (unused ?p)) (available ?p) (wood ?p ?w) (surface_condition ?p ?surface) (colour ?p natural) (treatment ?p untreated) (boardsize ?b ?size_after)))
 (:action do_saw_large
  :parameters ( ?b - board ?p - part ?m_5 - saw ?w - awood ?surface - surface ?size_before - aboardsize ?s1 - aboardsize ?s2 - aboardsize ?size_after - aboardsize)
  :precondition (and (unused ?p) (goalsize ?p large) (available ?b) (wood ?b ?w) (surface_condition ?b ?surface) (boardsize ?b ?size_before) (boardsize_successor ?size_after ?s1) (boardsize_successor ?s1 ?s2) (boardsize_successor ?s2 ?size_before))
  :effect (and (increase (total_cost) 30) (not (unused ?p)) (available ?p) (wood ?p ?w) (surface_condition ?p ?surface) (colour ?p natural) (treatment ?p untreated) (boardsize ?b ?size_after)))
)
