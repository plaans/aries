(define (problem blocks_4_0-problem)
 (:domain blocks_4_0-domain)
 (:objects
   d b a c - object
 )
 (:init (clear c) (clear a) (clear b) (clear d) (ontable c) (ontable a) (ontable b) (ontable d) (handempty))
 (:goal (and (on d c) (on c b) (on b a)))
)
