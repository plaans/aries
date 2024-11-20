(define (problem p-problem)
 (:domain p-domain)
 (:objects
   city_loc_0 city_loc_1 city_loc_2 - location
   truck_0 - vehicle
   package_0 package_1 - package
   capacity_0 capacity_1 - capacity_number
 )
 (:htn
  :subtasks (and
    (_t1863 (deliver package_0 city_loc_0))
    (_t1864 (deliver package_1 city_loc_2)))
  :ordering (and
    ))
 (:init (capacity_predecessor capacity_0 capacity_1) (road city_loc_0 city_loc_1) (road city_loc_1 city_loc_0) (road city_loc_1 city_loc_2) (road city_loc_2 city_loc_1) (at_ package_0 city_loc_1) (at_ package_1 city_loc_1) (at_ truck_0 city_loc_2) (capacity truck_0 capacity_1))
 (:goal (and ))
)
