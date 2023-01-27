(define (problem cooking1)
    (:domain cooking)
    (:objects
        eggs oil salt - ingredient
        fridge_zone pantry_zone watertap_zone recharge_zone cooking_zone - zone
        r2d2 - robot
        omelette - dish
    )
    (:init
        (is_cooking_zone cooking_zone)
        (is_fridge_zone fridge_zone)
        (is_pantry_zone pantry_zone)
        (is_watertap_zone watertap_zone)
        (is_recharge_zone recharge_zone)

        (ingredient_at eggs fridge_zone)
        (ingredient_at oil pantry_zone)
        (ingredient_at salt pantry_zone)

        (is_egg eggs)
        (is_oil oil)
        (is_salt salt)
        (is_omelette omelette)

        (robot_at r2d2 cooking_zone)
        (battery_full r2d2)
    )

    (:goal
        (and
            (dish_prepared omelette)
        )
    )

)

;;set instance eggs3 eggs
;;set instance oil2 oil
;;set instance salt1 salt
;;set instance fridge_zone zone
;;set instance pantry_zone zone
;;set instance watertap_zone zone
;;set instance recharge_zone zone
;;set instance cooking_zone zone
;;
;;set instance r2d2 robot
;;set instance ommelete1 omelette
;;
;;set predicate  (is_cooking_zone cooking_zone)
;;set predicate  (is_fridge_zone fridge_zone)
;;set predicate  (is_pantry_zone pantry_zone)
;;set predicate  (is_watertap_zone watertap_zone)
;;set predicate  (is_recharge_zone recharge_zone)
;;set predicate  (ingredient_at eggs3 fridge_zone)
;;set predicate  (ingredient_at oil2 pantry_zone)
;;set predicate  (ingredient_at salt1 pantry_zone)
;;set predicate  (robot_at r2d2 cooking_zone)
;;set predicate  (battery_full r2d2)
;;
;;set goal (and (dish_prepared ommelete1))
;;