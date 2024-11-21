(define (domain p-domain)
 (:requirements :strips :typing :hierarchy)
 (:types
    location target locatable capacity_number - object
    vehicle package - locatable
 )
 (:predicates (road ?l1 - location ?l2 - location) (at_ ?x - locatable ?v - location) (in ?x_0 - package ?v_0 - vehicle) (capacity ?v_0 - vehicle ?s1 - capacity_number) (capacity_predecessor ?s1 - capacity_number ?s2 - capacity_number))
 (:task deliver
  :parameters ( ?p - package ?l - location))
 (:task get_to
  :parameters ( ?v_0 - vehicle ?l - location))
 (:task load
  :parameters ( ?v_0 - vehicle ?l - location ?p - package))
 (:task unload
  :parameters ( ?v_0 - vehicle ?l - location ?p - package))
 (:method m_deliver
  :parameters ( ?p - package ?l1 - location ?l2 - location ?v_0 - vehicle)
  :task (deliver ?p ?l2)
  :ordered-subtasks (and
    (_t1853 (get_to ?v_0 ?l1))
    (_t1854 (load ?v_0 ?l1 ?p))
    (_t1855 (get_to ?v_0 ?l2))
    (_t1856 (unload ?v_0 ?l2 ?p))))
 (:method m_unload
  :parameters ( ?v_0 - vehicle ?l - location ?p - package ?s1 - capacity_number ?s2 - capacity_number)
  :task (unload ?v_0 ?l ?p)
  :ordered-subtasks (and
    (_t1857 (drop ?v_0 ?l ?p ?s1 ?s2))))
 (:method m_load
  :parameters ( ?v_0 - vehicle ?l - location ?p - package ?s1 - capacity_number ?s2 - capacity_number)
  :task (load ?v_0 ?l ?p)
  :ordered-subtasks (and
    (_t1858 (pick_up ?v_0 ?l ?p ?s1 ?s2))))
 (:method m_drive_to
  :parameters ( ?v_0 - vehicle ?l1 - location ?l2 - location)
  :task (get_to ?v_0 ?l2)
  :ordered-subtasks (and
    (_t1859 (drive ?v_0 ?l1 ?l2))))
 (:method m_drive_to_via
  :parameters ( ?v_0 - vehicle ?l2 - location ?l3 - location)
  :task (get_to ?v_0 ?l3)
  :ordered-subtasks (and
    (_t1860 (get_to ?v_0 ?l2))
    (_t1861 (drive ?v_0 ?l2 ?l3))))
 (:method m_i_am_there
  :parameters ( ?v_0 - vehicle ?l - location)
  :task (get_to ?v_0 ?l)
  :ordered-subtasks (and
    (_t1862 (noop ?v_0 ?l))))
 (:action drive
  :parameters ( ?v_0 - vehicle ?l1 - location ?l2 - location)
  :precondition (and (at_ ?v_0 ?l1) (road ?l1 ?l2))
  :effect (and (not (at_ ?v_0 ?l1)) (at_ ?v_0 ?l2)))
 (:action noop
  :parameters ( ?v_0 - vehicle ?l2 - location)
  :precondition (and (at_ ?v_0 ?l2)))
 (:action pick_up
  :parameters ( ?v_0 - vehicle ?l - location ?p - package ?s1 - capacity_number ?s2 - capacity_number)
  :precondition (and (at_ ?v_0 ?l) (at_ ?p ?l) (capacity_predecessor ?s1 ?s2) (capacity ?v_0 ?s2))
  :effect (and (not (at_ ?p ?l)) (in ?p ?v_0) (capacity ?v_0 ?s1) (not (capacity ?v_0 ?s2))))
 (:action drop
  :parameters ( ?v_0 - vehicle ?l - location ?p - package ?s1 - capacity_number ?s2 - capacity_number)
  :precondition (and (at_ ?v_0 ?l) (in ?p ?v_0) (capacity_predecessor ?s1 ?s2) (capacity ?v_0 ?s1))
  :effect (and (not (in ?p ?v_0)) (at_ ?p ?l) (capacity ?v_0 ?s2) (not (capacity ?v_0 ?s1))))
)
