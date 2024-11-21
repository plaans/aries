(define (problem gripper_x_1-problem)
 (:domain gripper_x_1-domain)
 (:objects
   rooma roomb - room
   ball4 ball3 ball2 ball1 - ball
   left right - gripper
 )
 (:init (at_robby rooma) (free left) (free right) (at_ ball4 rooma) (at_ ball3 rooma) (at_ ball2 rooma) (at_ ball1 rooma))
 (:goal (and (at_ ball4 roomb) (at_ ball3 roomb) (at_ ball2 roomb) (at_ ball1 roomb)))
)
