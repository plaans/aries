(define (problem delivery_x_1-problem)
 (:domain delivery_x_1-domain)
 (:objects
   rooma roomb roomc - room
   item4 item3 item2 item1 - item
   left1 right1 left2 right2 - arm
   bot1 bot2 - bot
 )
 (:init (= (weight item4) 1) (= (weight item3) 1) (= (weight item2) 1) (= (weight item1) 1) (at_bot bot1 rooma) (at_bot bot2 rooma) (free left1) (free right1) (free left2) (free right2) (mount left1 bot1) (mount right1 bot1) (mount left2 bot2) (mount right2 bot2) (at_ item4 rooma) (at_ item3 rooma) (at_ item2 rooma) (at_ item1 rooma) (door rooma roomb) (door roomb rooma) (door rooma roomc) (door roomc rooma) (= (current_load bot1) 0) (= (load_limit bot1) 4) (= (current_load bot2) 0) (= (load_limit bot2) 4) (= (cost) 0))
 (:goal (and (at_ item4 roomb) (at_ item3 roomb) (at_ item2 roomc) (at_ item1 roomc)))
 (:metric minimize (cost))
)
