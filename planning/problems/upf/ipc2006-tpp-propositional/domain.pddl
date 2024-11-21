(define (domain tpp-domain)
 (:requirements :strips :typing)
 (:types
    place locatable level - object
    truck goods - locatable
    depot market - place
 )
 (:predicates (loaded ?g - goods ?t - truck ?l - level) (ready_to_load ?g - goods ?m - market ?l - level) (stored ?g - goods ?l - level) (on_sale ?g - goods ?m - market ?l - level) (next ?l1 - level ?l2 - level) (at_ ?t - truck ?p - place) (connected ?p1 - place ?p2 - place))
 (:action drive
  :parameters ( ?t - truck ?from - place ?to - place)
  :precondition (and (at_ ?t ?from) (connected ?from ?to))
  :effect (and (not (at_ ?t ?from)) (at_ ?t ?to)))
 (:action load
  :parameters ( ?g - goods ?t - truck ?m - market ?l1 - level ?l2 - level ?l3 - level ?l4 - level)
  :precondition (and (at_ ?t ?m) (loaded ?g ?t ?l3) (ready_to_load ?g ?m ?l2) (next ?l2 ?l1) (next ?l4 ?l3))
  :effect (and (loaded ?g ?t ?l4) (not (loaded ?g ?t ?l3)) (ready_to_load ?g ?m ?l1) (not (ready_to_load ?g ?m ?l2))))
 (:action unload
  :parameters ( ?g - goods ?t - truck ?d - depot ?l1 - level ?l2 - level ?l3 - level ?l4 - level)
  :precondition (and (at_ ?t ?d) (loaded ?g ?t ?l2) (stored ?g ?l3) (next ?l2 ?l1) (next ?l4 ?l3))
  :effect (and (loaded ?g ?t ?l1) (not (loaded ?g ?t ?l2)) (stored ?g ?l4) (not (stored ?g ?l3))))
 (:action buy
  :parameters ( ?t - truck ?g - goods ?m - market ?l1 - level ?l2 - level ?l3 - level ?l4 - level)
  :precondition (and (at_ ?t ?m) (on_sale ?g ?m ?l2) (ready_to_load ?g ?m ?l3) (next ?l2 ?l1) (next ?l4 ?l3))
  :effect (and (on_sale ?g ?m ?l1) (not (on_sale ?g ?m ?l2)) (ready_to_load ?g ?m ?l4) (not (ready_to_load ?g ?m ?l3))))
)
