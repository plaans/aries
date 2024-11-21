(define (domain freecell_2_1-domain)
 (:requirements :strips :typing)
 (:predicates (on ?c1 - object ?c2 - object) (incell ?c - object) (clear ?c - object) (cellspace ?n - object) (colspace ?n - object) (home ?c - object) (bottomcol ?c - object) (canstack ?c1 - object ?c2 - object) (suit ?c - object ?s - object) (value ?c - object ?v - object) (successor ?n1 - object ?n0 - object))
 (:action move
  :parameters ( ?card - object ?oldcard - object ?newcard - object)
  :precondition (and (clear ?card) (clear ?newcard) (canstack ?card ?newcard) (on ?card ?oldcard))
  :effect (and (on ?card ?newcard) (clear ?oldcard) (not (on ?card ?oldcard)) (not (clear ?newcard))))
 (:action move_b
  :parameters ( ?card - object ?newcard - object ?cols - object ?ncols - object)
  :precondition (and (clear ?card) (bottomcol ?card) (clear ?newcard) (canstack ?card ?newcard) (colspace ?cols) (successor ?ncols ?cols))
  :effect (and (on ?card ?newcard) (colspace ?ncols) (not (bottomcol ?card)) (not (clear ?newcard)) (not (colspace ?cols))))
 (:action sendtofree
  :parameters ( ?card - object ?oldcard - object ?cells - object ?ncells - object)
  :precondition (and (clear ?card) (on ?card ?oldcard) (cellspace ?cells) (successor ?cells ?ncells))
  :effect (and (incell ?card) (clear ?oldcard) (cellspace ?ncells) (not (on ?card ?oldcard)) (not (clear ?card)) (not (cellspace ?cells))))
 (:action sendtofree_b
  :parameters ( ?card - object ?cells - object ?ncells - object ?cols - object ?ncols - object)
  :precondition (and (clear ?card) (bottomcol ?card) (cellspace ?cells) (successor ?cells ?ncells) (colspace ?cols) (successor ?ncols ?cols))
  :effect (and (incell ?card) (colspace ?ncols) (cellspace ?ncells) (not (bottomcol ?card)) (not (clear ?card)) (not (colspace ?cols)) (not (cellspace ?cells))))
 (:action sendtonewcol
  :parameters ( ?card - object ?oldcard - object ?cols - object ?ncols - object)
  :precondition (and (clear ?card) (colspace ?cols) (successor ?cols ?ncols) (on ?card ?oldcard))
  :effect (and (bottomcol ?card) (clear ?oldcard) (colspace ?ncols) (not (on ?card ?oldcard)) (not (colspace ?cols))))
 (:action sendtohome
  :parameters ( ?card - object ?oldcard - object ?suit - object ?vcard - object ?homecard - object ?vhomecard - object)
  :precondition (and (clear ?card) (on ?card ?oldcard) (home ?homecard) (suit ?card ?suit) (suit ?homecard ?suit) (value ?card ?vcard) (value ?homecard ?vhomecard) (successor ?vcard ?vhomecard))
  :effect (and (home ?card) (clear ?oldcard) (not (on ?card ?oldcard)) (not (home ?homecard)) (not (clear ?card))))
 (:action sendtohome_b
  :parameters ( ?card - object ?suit - object ?vcard - object ?homecard - object ?vhomecard - object ?cols - object ?ncols - object)
  :precondition (and (clear ?card) (bottomcol ?card) (home ?homecard) (suit ?card ?suit) (suit ?homecard ?suit) (value ?card ?vcard) (value ?homecard ?vhomecard) (successor ?vcard ?vhomecard) (colspace ?cols) (successor ?ncols ?cols))
  :effect (and (home ?card) (colspace ?ncols) (not (home ?homecard)) (not (clear ?card)) (not (bottomcol ?card)) (not (colspace ?cols))))
 (:action homefromfreecell
  :parameters ( ?card - object ?suit - object ?vcard - object ?homecard - object ?vhomecard - object ?cells - object ?ncells - object)
  :precondition (and (incell ?card) (home ?homecard) (suit ?card ?suit) (suit ?homecard ?suit) (value ?card ?vcard) (value ?homecard ?vhomecard) (successor ?vcard ?vhomecard) (cellspace ?cells) (successor ?ncells ?cells))
  :effect (and (home ?card) (cellspace ?ncells) (not (incell ?card)) (not (cellspace ?cells)) (not (home ?homecard))))
 (:action colfromfreecell
  :parameters ( ?card - object ?newcard - object ?cells - object ?ncells - object)
  :precondition (and (incell ?card) (clear ?newcard) (canstack ?card ?newcard) (cellspace ?cells) (successor ?ncells ?cells))
  :effect (and (cellspace ?ncells) (clear ?card) (on ?card ?newcard) (not (incell ?card)) (not (cellspace ?cells)) (not (clear ?newcard))))
 (:action newcolfromfreecell
  :parameters ( ?card - object ?cols - object ?ncols - object ?cells - object ?ncells - object)
  :precondition (and (incell ?card) (colspace ?cols) (cellspace ?cells) (successor ?cols ?ncols) (successor ?ncells ?cells))
  :effect (and (bottomcol ?card) (clear ?card) (colspace ?ncols) (cellspace ?ncells) (not (incell ?card)) (not (colspace ?cols)) (not (cellspace ?cells))))
)
