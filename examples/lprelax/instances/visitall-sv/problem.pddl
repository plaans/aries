(define (problem grid-2)
	(:domain grid-visit-all)
	(:objects 
		loc-x0
		loc-x1
		loc-x2
		loc-x3
		loc-x4
	- place 
			
	)
	(:init
		(= (at) loc-x0)
		(visited loc-x0)
		(connected loc-x0 loc-x1)
		(connected loc-x1 loc-x0)
		(connected loc-x1 loc-x2)
		(connected loc-x2 loc-x1)
		(connected loc-x2 loc-x3)
		(connected loc-x3 loc-x2)
		(connected loc-x3 loc-x4)
		(connected loc-x4 loc-x3)
	)
	(:goal
		(and 
			(visited loc-x0)
			(visited loc-x1)
			(visited loc-x2)
			(visited loc-x3)
			(visited loc-x4)
		)
	)
)