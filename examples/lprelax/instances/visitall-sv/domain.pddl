(define (domain grid-visit-all)
(:requirements :typing)
(:types        place - object)
(:predicates (connected ?x ?y - place)
	     (visited ?x - place)
)
(:functions (at) - place)
	
(:action move
:parameters (?curpos ?nextpos - place)
:precondition (and (= (at) ?curpos) 
  (connected ?curpos ?nextpos))
:effect (and (= (at) ?nextpos)
 (visited ?nextpos))
)

)
