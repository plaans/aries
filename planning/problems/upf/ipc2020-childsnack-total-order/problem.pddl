(define (problem prob_snack-problem)
 (:domain prob_snack-domain)
 (:objects
   child1 child2 child3 child4 child5 child6 child7 child8 child9 child10 - child
   bread1 bread2 bread3 bread4 bread5 bread6 bread7 bread8 bread9 bread10 - bread_portion
   content1 content2 content3 content4 content5 content6 content7 content8 content9 content10 - content_portion
   sandw1 sandw2 sandw3 sandw4 sandw5 sandw6 sandw7 sandw8 sandw9 sandw10 sandw11 sandw12 sandw13 - sandwich
   tray1 tray2 tray3 - tray
   table1 table2 table3 - place
 )
 (:htn
  :ordered-subtasks (and
    (task1 (serve child1))
    (task2 (serve child2))
    (task3 (serve child3))
    (task4 (serve child4))
    (task5 (serve child5))
    (task6 (serve child6))
    (task7 (serve child7))
    (task8 (serve child8))
    (task9 (serve child9))
    (task10 (serve child10))))
 (:init (at_ tray1 kitchen) (at_ tray2 kitchen) (at_ tray3 kitchen) (at_kitchen_bread bread1) (at_kitchen_bread bread2) (at_kitchen_bread bread3) (at_kitchen_bread bread4) (at_kitchen_bread bread5) (at_kitchen_bread bread6) (at_kitchen_bread bread7) (at_kitchen_bread bread8) (at_kitchen_bread bread9) (at_kitchen_bread bread10) (at_kitchen_content content1) (at_kitchen_content content2) (at_kitchen_content content3) (at_kitchen_content content4) (at_kitchen_content content5) (at_kitchen_content content6) (at_kitchen_content content7) (at_kitchen_content content8) (at_kitchen_content content9) (at_kitchen_content content10) (no_gluten_bread bread2) (no_gluten_bread bread9) (no_gluten_bread bread4) (no_gluten_bread bread8) (no_gluten_content content2) (no_gluten_content content8) (no_gluten_content content4) (no_gluten_content content1) (allergic_gluten child1) (allergic_gluten child10) (allergic_gluten child3) (allergic_gluten child4) (not_allergic_gluten child2) (not_allergic_gluten child5) (not_allergic_gluten child6) (not_allergic_gluten child7) (not_allergic_gluten child8) (not_allergic_gluten child9) (waiting child1 table2) (waiting child2 table1) (waiting child3 table1) (waiting child4 table2) (waiting child5 table3) (waiting child6 table3) (waiting child7 table3) (waiting child8 table2) (waiting child9 table1) (waiting child10 table3) (notexist sandw1) (notexist sandw2) (notexist sandw3) (notexist sandw4) (notexist sandw5) (notexist sandw6) (notexist sandw7) (notexist sandw8) (notexist sandw9) (notexist sandw10) (notexist sandw11) (notexist sandw12) (notexist sandw13))
 (:goal (and (served child1) (served child2) (served child3) (served child4) (served child5) (served child6) (served child7) (served child8) (served child9) (served child10)))
)
