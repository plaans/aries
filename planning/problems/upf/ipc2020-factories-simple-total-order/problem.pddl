(define (problem generated-problem)
 (:domain generated-domain)
 (:objects
   last_factory factory_0 factory_1 - factory
   resource_0 resource_1 resource_f_1_0 - resource
   location_0 location_1 last_location - location
 )
 (:htn
  :ordered-subtasks (and
    (_t293 (construct_factory last_factory last_location))))
 (:init (truck_at location_0) (factory_at factory_0 location_0) (factory_constructed factory_0) (factory_without_demands factory_0) (produces factory_0 resource_0) (demands last_factory resource_1) (location_free last_location) (connected location_1 location_0) (connected location_0 location_1) (produces factory_1 resource_1) (location_free location_1) (demands factory_1 resource_0) (fuses resource_f_1_0 resource_0 resource_0) (connected location_1 last_location) (connected last_location location_1))
 (:goal (and ))
)
