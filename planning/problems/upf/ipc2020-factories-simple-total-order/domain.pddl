(define (domain generated-domain)
 (:requirements :strips :typing :negative-preconditions :hierarchy :method-preconditions)
 (:types factory resource location)
 (:predicates (fuses ?r - resource ?r1 - resource ?r2 - resource) (demands ?f - factory ?r - resource) (factory_without_demands ?f - factory) (produces ?f - factory ?r - resource) (resource_at ?r - resource ?l - location) (factory_at ?f - factory ?l - location) (factory_constructed ?f - factory) (location_free ?l - location) (resource_in_truck ?r - resource) (truck_at ?l - location) (connected ?l1 - location ?l2 - location))
 (:task construct_factory
  :parameters ( ?f - factory ?l - location))
 (:task get_resource
  :parameters ( ?r - resource ?l - location))
 (:task produce_resource
  :parameters ( ?r - resource))
 (:task deliver_resource
  :parameters ( ?r - resource ?l - location))
 (:task goto
  :parameters ( ?l - location))
 (:method m_factory_already_constructed
  :parameters ( ?f - factory ?l - location)
  :task (construct_factory ?f ?l)
  :precondition (and (factory_at ?f ?l)))
 (:method m_construct_factory
  :parameters ( ?f - factory ?r - resource ?l - location)
  :task (construct_factory ?f ?l)
  :precondition (and (demands ?f ?r) (location_free ?l) (not (factory_constructed ?f)))
  :ordered-subtasks (and
    (_t276 (get_resource ?r ?l))
    (_t277 (construct ?f ?r ?l))))
 (:method m_resource_there
  :parameters ( ?r - resource ?l - location)
  :task (get_resource ?r ?l)
  :precondition (and (resource_at ?r ?l)))
 (:method m_get_resources_and_fuse
  :parameters ( ?r - resource ?r1 - resource ?r2 - resource ?l - location)
  :task (get_resource ?r ?l)
  :precondition (and (fuses ?r ?r1 ?r2))
  :ordered-subtasks (and
    (_t278 (get_resource ?r1 ?l))
    (_t279 (get_resource ?r2 ?l))
    (_t280 (fuse ?r ?r1 ?r2 ?l))))
 (:method m_get_resource
  :parameters ( ?r - resource ?f - factory ?fl - location ?l - location)
  :task (get_resource ?r ?l)
  :precondition (and (produces ?f ?r))
  :ordered-subtasks (and
    (_t281 (construct_factory ?f ?fl))
    (_t282 (produce_resource ?r))
    (_t283 (deliver_resource ?r ?l))))
 (:method m_produce_resource
  :parameters ( ?r - resource ?f - factory ?l - location)
  :task (produce_resource ?r)
  :precondition (and (produces ?f ?r) (factory_at ?f ?l) (factory_without_demands ?f))
  :ordered-subtasks (and
    (_t284 (produce_without_demands ?r ?f ?l))))
 (:method m_get_and_produce_resource
  :parameters ( ?r - resource ?rd - resource ?f - factory ?l - location)
  :task (produce_resource ?r)
  :precondition (and (produces ?f ?r) (demands ?f ?rd) (factory_at ?f ?l))
  :ordered-subtasks (and
    (_t285 (get_resource ?rd ?l))
    (_t286 (produce ?r ?rd ?f ?l))))
 (:method m_deliver_resource
  :parameters ( ?r - resource ?ls - location ?le - location)
  :task (deliver_resource ?r ?le)
  :precondition (and (resource_at ?r ?ls))
  :ordered-subtasks (and
    (_t287 (goto ?ls))
    (_t288 (pickup ?r ?ls))
    (_t289 (goto ?le))
    (_t290 (drop ?r ?le))))
 (:method m_goto
  :parameters ( ?l1 - location ?l2 - location ?le - location)
  :task (goto ?le)
  :precondition (and (truck_at ?l1) (connected ?l1 ?l2))
  :ordered-subtasks (and
    (_t291 (move ?l1 ?l2))
    (_t292 (goto ?le))))
 (:method m_already_there
  :parameters ( ?l - location)
  :task (goto ?l)
  :precondition (and (truck_at ?l)))
 (:action construct
  :parameters ( ?f - factory ?r - resource ?l - location)
  :precondition (and (location_free ?l) (demands ?f ?r) (resource_at ?r ?l))
  :effect (and (not (resource_at ?r ?l)) (not (location_free ?l)) (factory_at ?f ?l) (factory_constructed ?f)))
 (:action fuse
  :parameters ( ?r - resource ?r1 - resource ?r2 - resource ?l - location)
  :precondition (and (fuses ?r ?r1 ?r2) (resource_at ?r1 ?l) (resource_at ?r2 ?l))
  :effect (and (not (resource_at ?r1 ?l)) (not (resource_at ?r2 ?l)) (resource_at ?r ?l)))
 (:action produce_without_demands
  :parameters ( ?r - resource ?f - factory ?l - location)
  :precondition (and (produces ?f ?r) (factory_without_demands ?f) (factory_at ?f ?l))
  :effect (and (resource_at ?r ?l)))
 (:action produce
  :parameters ( ?r - resource ?rd - resource ?f - factory ?l - location)
  :precondition (and (produces ?f ?r) (demands ?f ?rd) (factory_at ?f ?l) (resource_at ?rd ?l))
  :effect (and (not (resource_at ?rd ?l)) (resource_at ?r ?l)))
 (:action pickup
  :parameters ( ?r - resource ?l - location)
  :precondition (and (resource_at ?r ?l) (truck_at ?l))
  :effect (and (not (resource_at ?r ?l)) (resource_in_truck ?r)))
 (:action drop
  :parameters ( ?r - resource ?l - location)
  :precondition (and (truck_at ?l) (resource_in_truck ?r))
  :effect (and (not (resource_in_truck ?r)) (resource_at ?r ?l)))
 (:action move
  :parameters ( ?l1 - location ?l2 - location)
  :precondition (and (truck_at ?l1) (connected ?l1 ?l2))
  :effect (and (not (truck_at ?l1)) (truck_at ?l2)))
)
