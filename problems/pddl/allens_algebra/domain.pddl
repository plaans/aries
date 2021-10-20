;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Captures possible relations between 2 intervals regarding Allen's algebra
;;;
;;; Meets relation is not implemented correctly! (there may exist a gap)

(define (domain allen-algebra)
    (:requirements :typing :durative-actions)
    (:types
        interval
    )
    (:predicates
        (started ?i - interval)
        (ended ?i - interval)
        (not-started ?i - interval) ;; duplicates used to preserve positive preconditions
        (not-ended ?i - interval) ;; 

        (before ?i1 ?i2 - interval) ;; desired goal conditions
        (meets ?i1 ?i2 - interval)
        (overlaps ?i1 ?i2 - interval)
        (starts ?i1 ?i2 - interval)
        (during ?i1 ?i2 - interval)
        (finishes ?i1 ?i2 - interval)
        (equal ?i1 ?i2 - interval)
    )
    (:functions
        (length ?i - interval)
    )

    ;;; Apply an interval
    (:durative-action apply-interval
        :parameters (?i - interval)
        :duration (= ?duration (length ?i))
        :condition (and
            (at start (not-started ?i))
        )
        :effect (and
            (at start (started ?i))
            (at start (not (not-started ?i)))
            (at end (ended ?i))
            (at end (not (not-ended ?i)))
        )
    )
)