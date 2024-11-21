(define (domain prob_snack-domain)
 (:requirements :strips :typing :negative-preconditions :hierarchy :method-preconditions)
 (:types child bread_portion content_portion sandwich tray place)
 (:constants
   kitchen - place
 )
 (:predicates (at_kitchen_bread ?b - bread_portion) (at_kitchen_content ?c - content_portion) (at_kitchen_sandwich ?s - sandwich) (no_gluten_bread ?b - bread_portion) (no_gluten_content ?c - content_portion) (ontray ?s - sandwich ?t - tray) (no_gluten_sandwich ?s - sandwich) (allergic_gluten ?c_0 - child) (not_allergic_gluten ?c_0 - child) (served ?c_0 - child) (waiting ?c_0 - child ?p - place) (at_ ?t - tray ?p - place) (notexist ?s - sandwich))
 (:task serve
  :parameters ( ?c_0 - child))
 (:method m0_serve
  :parameters ( ?c_0 - child ?s - sandwich ?b - bread_portion ?cont - content_portion ?t - tray ?p2 - place)
  :task (serve ?c_0)
  :precondition (and (allergic_gluten ?c_0) (notexist ?s) (waiting ?c_0 ?p2) (no_gluten_bread ?b) (no_gluten_content ?cont))
  :ordered-subtasks (and
    (t1 (make_sandwich_no_gluten ?s ?b ?cont))
    (t2 (put_on_tray ?s ?t))
    (t3 (move_tray ?t kitchen ?p2))
    (t4 (serve_sandwich_no_gluten ?s ?c_0 ?t ?p2))
    (t5 (move_tray ?t ?p2 kitchen))))
 (:method m1_serve
  :parameters ( ?c_0 - child ?s - sandwich ?b - bread_portion ?cont - content_portion ?t - tray ?p2 - place)
  :task (serve ?c_0)
  :precondition (and (not_allergic_gluten ?c_0) (notexist ?s) (waiting ?c_0 ?p2) (not (no_gluten_bread ?b)) (not (no_gluten_content ?cont)))
  :ordered-subtasks (and
    (t1 (make_sandwich ?s ?b ?cont))
    (t2 (put_on_tray ?s ?t))
    (t3 (move_tray ?t kitchen ?p2))
    (t4 (serve_sandwich ?s ?c_0 ?t ?p2))
    (t5 (move_tray ?t ?p2 kitchen))))
 (:action make_sandwich_no_gluten
  :parameters ( ?s - sandwich ?b - bread_portion ?c - content_portion)
  :precondition (and (at_kitchen_bread ?b) (at_kitchen_content ?c) (no_gluten_bread ?b) (no_gluten_content ?c) (notexist ?s))
  :effect (and (not (at_kitchen_bread ?b)) (not (at_kitchen_content ?c)) (at_kitchen_sandwich ?s) (no_gluten_sandwich ?s) (not (notexist ?s))))
 (:action make_sandwich
  :parameters ( ?s - sandwich ?b - bread_portion ?c - content_portion)
  :precondition (and (at_kitchen_bread ?b) (at_kitchen_content ?c) (notexist ?s))
  :effect (and (not (at_kitchen_bread ?b)) (not (at_kitchen_content ?c)) (at_kitchen_sandwich ?s) (not (notexist ?s))))
 (:action put_on_tray
  :parameters ( ?s - sandwich ?t - tray)
  :precondition (and (at_kitchen_sandwich ?s) (at_ ?t kitchen))
  :effect (and (not (at_kitchen_sandwich ?s)) (ontray ?s ?t)))
 (:action serve_sandwich_no_gluten
  :parameters ( ?s - sandwich ?c_0 - child ?t - tray ?p - place)
  :precondition (and (allergic_gluten ?c_0) (ontray ?s ?t) (waiting ?c_0 ?p) (no_gluten_sandwich ?s) (at_ ?t ?p))
  :effect (and (not (ontray ?s ?t)) (served ?c_0)))
 (:action serve_sandwich
  :parameters ( ?s - sandwich ?c_0 - child ?t - tray ?p - place)
  :precondition (and (not_allergic_gluten ?c_0) (waiting ?c_0 ?p) (ontray ?s ?t) (at_ ?t ?p))
  :effect (and (not (ontray ?s ?t)) (served ?c_0)))
 (:action move_tray
  :parameters ( ?t - tray ?p1 - place ?p2 - place)
  :precondition (and (at_ ?t ?p1))
  :effect (and (not (at_ ?t ?p1)) (at_ ?t ?p2)))
 (:action nop
  :parameters ())
)
