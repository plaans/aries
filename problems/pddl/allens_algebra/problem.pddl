(define (problem aa-before-2)
    (:domain allen-algebra)
    (:objects
        i1 i2 - interval
    )
    (:init
        (not-started i1)
        (not-ended i1)
        (not-started i2)
        (not-ended i2)
        (= (length i1) 5)
        (= (length i2) 5)
    )
    (:goal
        (and
            (before i1 i2)
        )
    )
)