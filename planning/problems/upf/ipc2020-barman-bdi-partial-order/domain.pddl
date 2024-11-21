(define (domain p_1_2_2-domain)
 (:requirements :strips :typing :negative-preconditions :equality :hierarchy :method-preconditions)
 (:types
    anything - object
    container dispenser level beverage hand - anything
    ingredient cocktail - beverage
    shot shaker - container
 )
 (:predicates (clean ?p0 - container) (cocktailpart1 ?p0_0 - cocktail ?p1 - ingredient) (cocktailpart2 ?p0_0 - cocktail ?p1 - ingredient) (contains ?p0 - container ?p1_0 - beverage) (dispenses ?p0_1 - dispenser ?p1 - ingredient) (empty ?p0 - container) (handempty ?p0_2 - hand) (holding ?p0_2 - hand ?p1_1 - container) (ingredient_0 ?p0_3 - ingredient) (next ?p0_4 - level ?p1_2 - level) (ontable ?p0 - container) (shaked ?p0_5 - shaker) (shakeremptylevel ?p0_5 - shaker ?p1_2 - level) (shakerlevel ?p0_5 - shaker ?p1_2 - level) (unshaked ?p0_5 - shaker) (used ?p0 - container ?p1_0 - beverage))
 (:task achievecontainsshakeringredient
  :parameters ( ?x_0 - shaker ?x_1 - ingredient))
 (:task achievecleanshaker
  :parameters ( ?x_0 - shaker))
 (:task achievehandempty
  :parameters ( ?x_0_0 - hand))
 (:task achievecontainsshotingredient
  :parameters ( ?x_0_1 - shot ?x_1 - ingredient))
 (:task achievecontainsshakercocktail
  :parameters ( ?x_0 - shaker ?x_1_0 - cocktail))
 (:task dopourshakertoshot
  :parameters ( ?x_0 - shaker ?x_1_1 - shot ?x_2 - cocktail))
 (:task achieveontable
  :parameters ( ?x_0_2 - container))
 (:task achieveholding
  :parameters ( ?x_0_0 - hand ?x_1_2 - container))
 (:task achievecleanshot
  :parameters ( ?x_0_1 - shot))
 (:task achievecontainsshotcocktail
  :parameters ( ?x_0_1 - shot ?x_1_0 - cocktail))
 (:method makeandpourcocktail
  :parameters ( ?x_0_1 - shot ?x_1_0 - cocktail ?x_2_0 - shaker ?x_3 - hand)
  :task (achievecontainsshotcocktail ?x_0_1 ?x_1_0)
  :precondition (and (not (contains ?x_0_1 ?x_1_0)))
  :ordered-subtasks (and
    (_t22 (achievecontainsshakercocktail ?x_2_0 ?x_1_0))
    (_t23 (achievecleanshot ?x_0_1))
    (_t24 (achieveholding ?x_3 ?x_2_0))
    (_t25 (dopourshakertoshot ?x_2_0 ?x_0_1 ?x_1_0))))
 (:method makeandpourcocktailnull
  :parameters ( ?x_0_1 - shot ?x_1_0 - cocktail)
  :task (achievecontainsshotcocktail ?x_0_1 ?x_1_0)
  :precondition (and (contains ?x_0_1 ?x_1_0)))
 (:method makecocktail
  :parameters ( ?x_0 - shaker ?x_1_0 - cocktail ?x_2_1 - ingredient ?x_3 - hand ?x_4 - hand ?x_5 - ingredient)
  :task (achievecontainsshakercocktail ?x_0 ?x_1_0)
  :precondition (and (cocktailpart1 ?x_1_0 ?x_5) (cocktailpart2 ?x_1_0 ?x_2_1) (not (= ?x_4 ?x_3)))
  :ordered-subtasks (and
    (_t26 (achievecleanshaker ?x_0))
    (_t27 (achievecontainsshakeringredient ?x_0 ?x_5))
    (_t28 (achievecontainsshakeringredient ?x_0 ?x_2_1))
    (_t29 (achieveholding ?x_4 ?x_0))
    (_t30 (achievehandempty ?x_3))
    (_t31 (shake ?x_1_0 ?x_5 ?x_2_1 ?x_0 ?x_4 ?x_3))))
 (:method makecocktailnull
  :parameters ( ?x_0 - shaker ?x_1_0 - cocktail)
  :task (achievecontainsshakercocktail ?x_0 ?x_1_0)
  :precondition (and (contains ?x_0 ?x_1_0)))
 (:method addingredienttoemptyshaker
  :parameters ( ?x_0 - shaker ?x_1 - ingredient ?x_2_2 - level ?x_3_0 - level ?x_4_0 - shot ?x_5_0 - hand)
  :task (achievecontainsshakeringredient ?x_0 ?x_1)
  :precondition (and (empty ?x_0) (clean ?x_0) (shakerlevel ?x_0 ?x_2_2) (next ?x_2_2 ?x_3_0))
  :ordered-subtasks (and
    (_t32 (achievecontainsshotingredient ?x_4_0 ?x_1))
    (_t33 (achieveholding ?x_5_0 ?x_4_0))
    (_t34 (pour_shot_to_clean_shaker ?x_4_0 ?x_1 ?x_0 ?x_5_0 ?x_2_2 ?x_3_0))))
 (:method addingredienttousedshaker
  :parameters ( ?x_0 - shaker ?x_1 - ingredient ?x_2_2 - level ?x_3_0 - level ?x_4_0 - shot ?x_5_0 - hand)
  :task (achievecontainsshakeringredient ?x_0 ?x_1)
  :precondition (and (not (empty ?x_0)) (shakerlevel ?x_0 ?x_2_2) (next ?x_2_2 ?x_3_0))
  :ordered-subtasks (and
    (_t35 (achievecontainsshotingredient ?x_4_0 ?x_1))
    (_t36 (achieveholding ?x_5_0 ?x_4_0))
    (_t37 (pour_shot_to_used_shaker ?x_4_0 ?x_1 ?x_0 ?x_5_0 ?x_2_2 ?x_3_0))))
 (:method addingredienttoshakernull
  :parameters ( ?x_0 - shaker ?x_1 - ingredient)
  :task (achievecontainsshakeringredient ?x_0 ?x_1)
  :precondition (and (contains ?x_0 ?x_1)))
 (:method addingredienttoshot
  :parameters ( ?x_0_1 - shot ?x_1 - ingredient ?x_2_3 - dispenser ?x_3 - hand ?x_4 - hand)
  :task (achievecontainsshotingredient ?x_0_1 ?x_1)
  :precondition (and (not (contains ?x_0_1 ?x_1)) (dispenses ?x_2_3 ?x_1) (not (= ?x_4 ?x_3)))
  :ordered-subtasks (and
    (_t38 (achievecleanshot ?x_0_1))
    (_t39 (achieveholding ?x_4 ?x_0_1))
    (_t40 (achievehandempty ?x_3))
    (_t41 (fill_shot ?x_0_1 ?x_1 ?x_4 ?x_3 ?x_2_3))))
 (:method addingredienttoshotnull
  :parameters ( ?x_0_1 - shot ?x_1 - ingredient)
  :task (achievecontainsshotingredient ?x_0_1 ?x_1)
  :precondition (and (contains ?x_0_1 ?x_1)))
 (:method cleanfullshot
  :parameters ( ?x_0_1 - shot ?x_1_3 - hand ?x_2_4 - beverage ?x_3 - hand)
  :task (achievecleanshot ?x_0_1)
  :precondition (and (contains ?x_0_1 ?x_2_4) (not (= ?x_3 ?x_1_3)))
  :ordered-subtasks (and
    (_t42 (achieveholding ?x_3 ?x_0_1))
    (_t43 (empty_shot ?x_3 ?x_0_1 ?x_2_4))
    (_t44 (achievehandempty ?x_1_3))
    (_t45 (clean_shot ?x_0_1 ?x_2_4 ?x_3 ?x_1_3))))
 (:method cleanemptyshot
  :parameters ( ?x_0_1 - shot ?x_1_3 - hand ?x_2_4 - beverage ?x_3 - hand)
  :task (achievecleanshot ?x_0_1)
  :precondition (and (empty ?x_0_1) (used ?x_0_1 ?x_2_4) (not (= ?x_3 ?x_1_3)))
  :ordered-subtasks (and
    (_t46 (achieveholding ?x_3 ?x_0_1))
    (_t47 (achievehandempty ?x_1_3))
    (_t48 (clean_shot ?x_0_1 ?x_2_4 ?x_3 ?x_1_3))))
 (:method cleanshotnull
  :parameters ( ?x_0_1 - shot)
  :task (achievecleanshot ?x_0_1)
  :precondition (and (clean ?x_0_1)))
 (:method cleanemptyshaker
  :parameters ( ?x_0 - shaker ?x_1_3 - hand ?x_2_5 - hand)
  :task (achievecleanshaker ?x_0)
  :precondition (and (not (clean ?x_0)) (empty ?x_0) (not (= ?x_2_5 ?x_1_3)))
  :ordered-subtasks (and
    (_t49 (achieveholding ?x_2_5 ?x_0))
    (_t50 (achievehandempty ?x_1_3))
    (_t51 (clean_shaker ?x_0 ?x_2_5 ?x_1_3))))
 (:method cleanfullshaker
  :parameters ( ?x_0 - shaker ?x_1_4 - level ?x_2 - cocktail ?x_3 - hand ?x_4 - hand ?x_5_1 - level)
  :task (achievecleanshaker ?x_0)
  :precondition (and (contains ?x_0 ?x_2) (shaked ?x_0) (shakeremptylevel ?x_0 ?x_1_4) (shakerlevel ?x_0 ?x_5_1) (not (= ?x_4 ?x_3)))
  :ordered-subtasks (and
    (_t52 (achieveholding ?x_4 ?x_0))
    (_t53 (empty_shaker ?x_4 ?x_0 ?x_2 ?x_5_1 ?x_1_4))
    (_t54 (achievehandempty ?x_3))
    (_t55 (clean_shaker ?x_0 ?x_4 ?x_3))))
 (:method cleanshakernull
  :parameters ( ?x_0 - shaker)
  :task (achievecleanshaker ?x_0)
  :precondition (and (clean ?x_0)))
 (:method pickup
  :parameters ( ?x_0_0 - hand ?x_1_2 - container)
  :task (achieveholding ?x_0_0 ?x_1_2)
  :precondition (and (not (holding ?x_0_0 ?x_1_2)))
  :ordered-subtasks (and
    (_t56 (achievehandempty ?x_0_0))
    (_t57 (achieveontable ?x_1_2))
    (_t58 (grasp ?x_0_0 ?x_1_2))))
 (:method holdingnull
  :parameters ( ?x_0_0 - hand ?x_1_2 - container)
  :task (achieveholding ?x_0_0 ?x_1_2)
  :precondition (and (holding ?x_0_0 ?x_1_2)))
 (:method emptyhand
  :parameters ( ?x_0_0 - hand ?x_1_2 - container)
  :task (achievehandempty ?x_0_0)
  :precondition (and (holding ?x_0_0 ?x_1_2))
  :ordered-subtasks (and
    (_t59 (drop ?x_0_0 ?x_1_2))))
 (:method handemptynull
  :parameters ( ?x_0_0 - hand ?x_1_3 - hand)
  :task (achievehandempty ?x_0_0)
  :precondition (and (handempty ?x_1_3)))
 (:method putdown
  :parameters ( ?x_0_2 - container ?x_1_3 - hand)
  :task (achieveontable ?x_0_2)
  :precondition (and (holding ?x_1_3 ?x_0_2))
  :ordered-subtasks (and
    (_t60 (drop ?x_1_3 ?x_0_2))))
 (:method ontablenull
  :parameters ( ?x_0_2 - container)
  :task (achieveontable ?x_0_2)
  :precondition (and (ontable ?x_0_2)))
 (:method pour_shaker_to_shot_action
  :parameters ( ?x_0 - shaker ?x_1_1 - shot ?x_2 - cocktail ?x_3_0 - level ?x_4 - hand ?x_5_1 - level)
  :task (dopourshakertoshot ?x_0 ?x_1_1 ?x_2)
  :precondition (and (holding ?x_4 ?x_0) (shaked ?x_0) (empty ?x_1_1) (clean ?x_1_1) (contains ?x_0 ?x_2) (shakerlevel ?x_0 ?x_3_0) (next ?x_5_1 ?x_3_0))
  :ordered-subtasks (and
    (_t61 (pour_shaker_to_shot ?x_2 ?x_1_1 ?x_4 ?x_0 ?x_3_0 ?x_5_1))))
 (:action clean_shaker
  :parameters ( ?x_0 - shaker ?x_1_3 - hand ?x_2_5 - hand)
  :precondition (and (holding ?x_1_3 ?x_0) (empty ?x_0) (handempty ?x_2_5))
  :effect (and (clean ?x_0)))
 (:action clean_shot
  :parameters ( ?x_0_1 - shot ?x_1_5 - beverage ?x_2_5 - hand ?x_3 - hand)
  :precondition (and (holding ?x_2_5 ?x_0_1) (handempty ?x_3) (empty ?x_0_1) (used ?x_0_1 ?x_1_5))
  :effect (and (clean ?x_0_1) (not (used ?x_0_1 ?x_1_5))))
 (:action drop
  :parameters ( ?x_0_0 - hand ?x_1_2 - container)
  :precondition (and (holding ?x_0_0 ?x_1_2))
  :effect (and (ontable ?x_1_2) (handempty ?x_0_0) (not (holding ?x_0_0 ?x_1_2))))
 (:action empty_shaker
  :parameters ( ?x_0_0 - hand ?x_1_6 - shaker ?x_2 - cocktail ?x_3_0 - level ?x_4_1 - level)
  :precondition (and (holding ?x_0_0 ?x_1_6) (contains ?x_1_6 ?x_2) (shaked ?x_1_6) (shakeremptylevel ?x_1_6 ?x_4_1) (shakerlevel ?x_1_6 ?x_3_0))
  :effect (and (empty ?x_1_6) (unshaked ?x_1_6) (shakerlevel ?x_1_6 ?x_4_1) (not (contains ?x_1_6 ?x_2)) (not (shakerlevel ?x_1_6 ?x_3_0)) (not (shaked ?x_1_6))))
 (:action empty_shot
  :parameters ( ?x_0_0 - hand ?x_1_1 - shot ?x_2_4 - beverage)
  :precondition (and (holding ?x_0_0 ?x_1_1) (contains ?x_1_1 ?x_2_4))
  :effect (and (empty ?x_1_1) (not (contains ?x_1_1 ?x_2_4))))
 (:action fill_shot
  :parameters ( ?x_0_1 - shot ?x_1 - ingredient ?x_2_5 - hand ?x_3 - hand ?x_4_2 - dispenser)
  :precondition (and (holding ?x_2_5 ?x_0_1) (handempty ?x_3) (empty ?x_0_1) (clean ?x_0_1) (dispenses ?x_4_2 ?x_1))
  :effect (and (contains ?x_0_1 ?x_1) (used ?x_0_1 ?x_1) (not (clean ?x_0_1)) (not (empty ?x_0_1))))
 (:action grasp
  :parameters ( ?x_0_0 - hand ?x_1_2 - container)
  :precondition (and (ontable ?x_1_2) (handempty ?x_0_0))
  :effect (and (holding ?x_0_0 ?x_1_2) (not (handempty ?x_0_0)) (not (ontable ?x_1_2))))
 (:action pour_shaker_to_shot
  :parameters ( ?x_0_3 - cocktail ?x_1_1 - shot ?x_2_5 - hand ?x_3_1 - shaker ?x_4_1 - level ?x_5_1 - level)
  :precondition (and (holding ?x_2_5 ?x_3_1) (contains ?x_3_1 ?x_0_3) (shaked ?x_3_1) (clean ?x_1_1) (empty ?x_1_1) (shakerlevel ?x_3_1 ?x_4_1) (next ?x_5_1 ?x_4_1))
  :effect (and (contains ?x_1_1 ?x_0_3) (used ?x_1_1 ?x_0_3) (shakerlevel ?x_3_1 ?x_5_1) (not (clean ?x_1_1)) (not (empty ?x_1_1)) (not (shakerlevel ?x_3_1 ?x_4_1))))
 (:action pour_shot_to_clean_shaker
  :parameters ( ?x_0_1 - shot ?x_1 - ingredient ?x_2_0 - shaker ?x_3 - hand ?x_4_1 - level ?x_5_1 - level)
  :precondition (and (contains ?x_0_1 ?x_1) (empty ?x_2_0) (clean ?x_2_0) (holding ?x_3 ?x_0_1) (shakerlevel ?x_2_0 ?x_4_1) (next ?x_4_1 ?x_5_1))
  :effect (and (contains ?x_2_0 ?x_1) (shakerlevel ?x_2_0 ?x_5_1) (unshaked ?x_2_0) (empty ?x_0_1) (not (clean ?x_2_0)) (not (empty ?x_2_0)) (not (contains ?x_0_1 ?x_1)) (not (shakerlevel ?x_2_0 ?x_4_1))))
 (:action pour_shot_to_used_shaker
  :parameters ( ?x_0_1 - shot ?x_1 - ingredient ?x_2_0 - shaker ?x_3 - hand ?x_4_1 - level ?x_5_1 - level)
  :precondition (and (contains ?x_0_1 ?x_1) (unshaked ?x_2_0) (holding ?x_3 ?x_0_1) (shakerlevel ?x_2_0 ?x_4_1) (next ?x_4_1 ?x_5_1))
  :effect (and (contains ?x_2_0 ?x_1) (shakerlevel ?x_2_0 ?x_5_1) (empty ?x_0_1) (not (contains ?x_0_1 ?x_1)) (not (shakerlevel ?x_2_0 ?x_4_1))))
 (:action shake
  :parameters ( ?x_0_3 - cocktail ?x_1 - ingredient ?x_2_1 - ingredient ?x_3_1 - shaker ?x_4 - hand ?x_5_0 - hand)
  :precondition (and (handempty ?x_5_0) (holding ?x_4 ?x_3_1) (contains ?x_3_1 ?x_1) (contains ?x_3_1 ?x_2_1) (unshaked ?x_3_1) (cocktailpart1 ?x_0_3 ?x_1) (cocktailpart2 ?x_0_3 ?x_2_1))
  :effect (and (shaked ?x_3_1) (contains ?x_3_1 ?x_0_3) (not (unshaked ?x_3_1)) (not (contains ?x_3_1 ?x_1)) (not (contains ?x_3_1 ?x_2_1))))
)
