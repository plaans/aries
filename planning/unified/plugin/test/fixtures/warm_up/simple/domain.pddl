(define (domain simple-domain)
 (:requirements :strips)
 (:predicates (is_set ?x))
 (:action set
    :parameters (?x)
    :precondition ()
    :effect (and (is_set ?x))
 )
)