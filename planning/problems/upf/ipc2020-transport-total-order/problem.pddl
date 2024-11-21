(define (problem pfile01-problem)
 (:domain pfile01-domain)
 (:objects
   package_0 package_1 - package
   capacity_0 capacity_1 - capacity_number
   city_loc_0 city_loc_1 city_loc_2 - location
   truck_0 - vehicle
 )
 (:htn
  :ordered-subtasks (and
    (task0 (deliver package_0 city_loc_0))
    (task1 (deliver package_1 city_loc_2))))
 (:init (capacity_predecessor capacity_0 capacity_1) (road city_loc_0 city_loc_1) (road city_loc_1 city_loc_0) (road city_loc_1 city_loc_2) (road city_loc_2 city_loc_1) (at_ package_0 city_loc_1) (at_ package_1 city_loc_1) (at_ truck_0 city_loc_2) (capacity truck_0 capacity_1))
 (:goal (and ))
)
