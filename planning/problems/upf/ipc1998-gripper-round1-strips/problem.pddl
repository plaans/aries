(define (problem strips_gripper_x_1-problem)
 (:domain strips_gripper_x_1-domain)
 (:objects
   rooma roomb ball4 ball3 ball2 ball1 left right - object
 )
 (:init (room rooma) (room roomb) (ball ball4) (ball ball3) (ball ball2) (ball ball1) (at_robby rooma) (free left) (free right) (at_ ball4 rooma) (at_ ball3 rooma) (at_ ball2 rooma) (at_ ball1 rooma) (gripper left) (gripper right))
 (:goal (and (at_ ball4 roomb) (at_ ball3 roomb) (at_ ball2 roomb) (at_ ball1 roomb)))
)
