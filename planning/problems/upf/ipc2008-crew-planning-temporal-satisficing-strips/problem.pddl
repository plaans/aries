(define (problem crewplanning_1crew_1day_40utilization-problem)
 (:domain crewplanning_1crew_1day_40utilization-domain)
 (:objects
   mcs1 - medicalstate
   spaceshipfilter - filterstate
   c1 - crewmember
   d0 d1 d2 - day
   e1 - exerequipment
   rpcm1 - rpcm
 )
 (:init (currentday c1 d0) (done_sleep c1 d0) (available c1) (initiated d1) (next d0 d1) (next d1 d2) (unused e1))
 (:goal (and (done_sleep c1 d1) (initiated d2) (changed spaceshipfilter d1) (done_rpcm rpcm1 d1)))
 (:metric minimize (total-time))
)
