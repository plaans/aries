(define (domain pfile01-domain)
 (:requirements :strips :typing :hierarchy)
 (:types
    locatable capacity_number location target - object
    package vehicle - locatable
 )
 (:predicates (road ?arg0 - location ?arg1 - location) (at_ ?arg0_0 - locatable ?arg1 - location) (in ?arg0_1 - package ?arg1_0 - vehicle) (capacity ?arg0_2 - vehicle ?arg1_1 - capacity_number) (capacity_predecessor ?arg0_3 - capacity_number ?arg1_1 - capacity_number))
 (:task deliver
  :parameters ( ?p - package ?l - location))
 (:task get_to
  :parameters ( ?v - vehicle ?l - location))
 (:task load
  :parameters ( ?v - vehicle ?l - location ?p - package))
 (:task unload
  :parameters ( ?v - vehicle ?l - location ?p - package))
 (:method m_deliver_ordering_0
  :parameters ( ?l1 - location ?l2 - location ?p - package ?v - vehicle)
  :task (deliver ?p ?l2)
  :ordered-subtasks (and
    (task0 (get_to ?v ?l1))
    (task1 (load ?v ?l1 ?p))
    (task2 (get_to ?v ?l2))
    (task3 (unload ?v ?l2 ?p))))
 (:method m_unload_ordering_0
  :parameters ( ?l - location ?p - package ?s1 - capacity_number ?s2 - capacity_number ?v - vehicle)
  :task (unload ?v ?l ?p)
  :ordered-subtasks (and
    (task0 (drop ?v ?l ?p ?s1 ?s2))))
 (:method m_load_ordering_0
  :parameters ( ?l - location ?p - package ?s1 - capacity_number ?s2 - capacity_number ?v - vehicle)
  :task (load ?v ?l ?p)
  :ordered-subtasks (and
    (task0 (pick_up ?v ?l ?p ?s1 ?s2))))
 (:method m_drive_to_ordering_0
  :parameters ( ?l1 - location ?l2 - location ?v - vehicle)
  :task (get_to ?v ?l2)
  :ordered-subtasks (and
    (task0 (drive ?v ?l1 ?l2))))
 (:method m_drive_to_via_ordering_0
  :parameters ( ?l2 - location ?l3 - location ?v - vehicle)
  :task (get_to ?v ?l3)
  :ordered-subtasks (and
    (task0 (get_to ?v ?l2))
    (task1 (drive ?v ?l2 ?l3))))
 (:method m_i_am_there_ordering_0
  :parameters ( ?l - location ?v - vehicle)
  :task (get_to ?v ?l)
  :ordered-subtasks (and
    (task0 (noop ?v ?l))))
 (:action drive
  :parameters ( ?v - vehicle ?l1 - location ?l2 - location)
  :precondition (and (at_ ?v ?l1) (road ?l1 ?l2))
  :effect (and (not (at_ ?v ?l1)) (at_ ?v ?l2)))
 (:action noop
  :parameters ( ?v - vehicle ?l2 - location)
  :precondition (and (at_ ?v ?l2)))
 (:action pick_up
  :parameters ( ?v - vehicle ?l - location ?p - package ?s1 - capacity_number ?s2 - capacity_number)
  :precondition (and (at_ ?v ?l) (at_ ?p ?l) (capacity_predecessor ?s1 ?s2) (capacity ?v ?s2))
  :effect (and (not (at_ ?p ?l)) (in ?p ?v) (capacity ?v ?s1) (not (capacity ?v ?s2))))
 (:action drop
  :parameters ( ?v - vehicle ?l - location ?p - package ?s1 - capacity_number ?s2 - capacity_number)
  :precondition (and (at_ ?v ?l) (in ?p ?v) (capacity_predecessor ?s1 ?s2) (capacity ?v ?s1))
  :effect (and (not (in ?p ?v)) (at_ ?p ?l) (capacity ?v ?s2) (not (capacity ?v ?s1))))
)
