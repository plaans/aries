(define (domain scanalyzer3d_52-domain)
 (:requirements :strips :typing :action-costs)
 (:types segment car)
 (:predicates (on ?c - car ?s - segment) (analyzed ?c - car) (cycle_2 ?s1 - segment ?s2 - segment) (cycle_4 ?s1 - segment ?s2 - segment ?s3 - segment ?s4 - segment) (cycle_2_with_analysis ?s1 - segment ?s2 - segment) (cycle_4_with_analysis ?s1 - segment ?s2 - segment ?s3 - segment ?s4 - segment))
 (:functions (total-cost))
 (:action analyze_2
  :parameters ( ?s1 - segment ?s2 - segment ?c1 - car ?c2 - car)
  :precondition (and (cycle_2_with_analysis ?s1 ?s2) (on ?c1 ?s1) (on ?c2 ?s2))
  :effect (and (not (on ?c1 ?s1)) (not (on ?c2 ?s2)) (on ?c1 ?s2) (on ?c2 ?s1) (analyzed ?c1) (increase (total-cost) 3)))
 (:action analyze_4
  :parameters ( ?s1 - segment ?s2 - segment ?s3 - segment ?s4 - segment ?c1 - car ?c2 - car ?c3 - car ?c4 - car)
  :precondition (and (cycle_4_with_analysis ?s1 ?s2 ?s3 ?s4) (on ?c1 ?s1) (on ?c2 ?s2) (on ?c3 ?s3) (on ?c4 ?s4))
  :effect (and (not (on ?c1 ?s1)) (not (on ?c2 ?s2)) (not (on ?c3 ?s3)) (not (on ?c4 ?s4)) (on ?c1 ?s4) (on ?c2 ?s1) (on ?c3 ?s2) (on ?c4 ?s3) (analyzed ?c1) (increase (total-cost) 3)))
 (:action rotate_2
  :parameters ( ?s1 - segment ?s2 - segment ?c1 - car ?c2 - car)
  :precondition (and (cycle_2 ?s1 ?s2) (on ?c1 ?s1) (on ?c2 ?s2))
  :effect (and (not (on ?c1 ?s1)) (not (on ?c2 ?s2)) (on ?c1 ?s2) (on ?c2 ?s1) (increase (total-cost) 1)))
 (:action rotate_4
  :parameters ( ?s1 - segment ?s2 - segment ?s3 - segment ?s4 - segment ?c1 - car ?c2 - car ?c3 - car ?c4 - car)
  :precondition (and (cycle_4 ?s1 ?s2 ?s3 ?s4) (on ?c1 ?s1) (on ?c2 ?s2) (on ?c3 ?s3) (on ?c4 ?s4))
  :effect (and (not (on ?c1 ?s1)) (not (on ?c2 ?s2)) (not (on ?c3 ?s3)) (not (on ?c4 ?s4)) (on ?c1 ?s4) (on ?c2 ?s1) (on ?c3 ?s2) (on ?c4 ?s3) (increase (total-cost) 1)))
)
