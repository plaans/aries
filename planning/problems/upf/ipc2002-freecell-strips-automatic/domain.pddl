(define (domain freecell2_4-domain)
 (:requirements :strips :typing)
 (:types card suitsort denomination)
 (:predicates (on ?c1 - card ?c2 - card) (incell ?c - card) (clear ?c - card) (cellspace ?n - denomination) (colspace ?n - denomination) (home ?c - card) (bottomcol ?c - card) (canstack ?c1 - card ?c2 - card) (suit ?c - card ?s - suitsort) (value ?c - card ?v - denomination) (successor ?n1 - denomination ?n0 - denomination))
 (:action move
  :parameters ( ?card - card ?oldcard - card ?newcard - card)
  :precondition (and (clear ?card) (clear ?newcard) (canstack ?card ?newcard) (on ?card ?oldcard))
  :effect (and (on ?card ?newcard) (clear ?oldcard) (not (on ?card ?oldcard)) (not (clear ?newcard))))
 (:action move_b
  :parameters ( ?card - card ?newcard - card ?cols - denomination ?ncols - denomination)
  :precondition (and (clear ?card) (bottomcol ?card) (clear ?newcard) (canstack ?card ?newcard) (colspace ?cols) (successor ?ncols ?cols))
  :effect (and (on ?card ?newcard) (colspace ?ncols) (not (bottomcol ?card)) (not (clear ?newcard)) (not (colspace ?cols))))
 (:action sendtofree
  :parameters ( ?card - card ?oldcard - card ?cells - denomination ?ncells - denomination)
  :precondition (and (clear ?card) (on ?card ?oldcard) (cellspace ?cells) (successor ?cells ?ncells))
  :effect (and (incell ?card) (clear ?oldcard) (cellspace ?ncells) (not (on ?card ?oldcard)) (not (clear ?card)) (not (cellspace ?cells))))
 (:action sendtofree_b
  :parameters ( ?card - card ?cells - denomination ?ncells - denomination ?cols - denomination ?ncols - denomination)
  :precondition (and (clear ?card) (bottomcol ?card) (cellspace ?cells) (successor ?cells ?ncells) (colspace ?cols) (successor ?ncols ?cols))
  :effect (and (incell ?card) (colspace ?ncols) (cellspace ?ncells) (not (bottomcol ?card)) (not (clear ?card)) (not (colspace ?cols)) (not (cellspace ?cells))))
 (:action sendtonewcol
  :parameters ( ?card - card ?oldcard - card ?cols - denomination ?ncols - denomination)
  :precondition (and (clear ?card) (colspace ?cols) (successor ?cols ?ncols) (on ?card ?oldcard))
  :effect (and (bottomcol ?card) (clear ?oldcard) (colspace ?ncols) (not (on ?card ?oldcard)) (not (colspace ?cols))))
 (:action sendtohome
  :parameters ( ?card - card ?oldcard - card ?suit - suitsort ?vcard - denomination ?homecard - card ?vhomecard - denomination)
  :precondition (and (clear ?card) (on ?card ?oldcard) (home ?homecard) (suit ?card ?suit) (suit ?homecard ?suit) (value ?card ?vcard) (value ?homecard ?vhomecard) (successor ?vcard ?vhomecard))
  :effect (and (home ?card) (clear ?oldcard) (not (on ?card ?oldcard)) (not (home ?homecard)) (not (clear ?card))))
 (:action sendtohome_b
  :parameters ( ?card - card ?suit - suitsort ?vcard - denomination ?homecard - card ?vhomecard - denomination ?cols - denomination ?ncols - denomination)
  :precondition (and (clear ?card) (bottomcol ?card) (home ?homecard) (suit ?card ?suit) (suit ?homecard ?suit) (value ?card ?vcard) (value ?homecard ?vhomecard) (successor ?vcard ?vhomecard) (colspace ?cols) (successor ?ncols ?cols))
  :effect (and (home ?card) (colspace ?ncols) (not (home ?homecard)) (not (clear ?card)) (not (bottomcol ?card)) (not (colspace ?cols))))
 (:action homefromfreecell
  :parameters ( ?card - card ?suit - suitsort ?vcard - denomination ?homecard - card ?vhomecard - denomination ?cells - denomination ?ncells - denomination)
  :precondition (and (incell ?card) (home ?homecard) (suit ?card ?suit) (suit ?homecard ?suit) (value ?card ?vcard) (value ?homecard ?vhomecard) (successor ?vcard ?vhomecard) (cellspace ?cells) (successor ?ncells ?cells))
  :effect (and (home ?card) (cellspace ?ncells) (not (incell ?card)) (not (cellspace ?cells)) (not (home ?homecard))))
 (:action colfromfreecell
  :parameters ( ?card - card ?newcard - card ?cells - denomination ?ncells - denomination)
  :precondition (and (incell ?card) (clear ?newcard) (canstack ?card ?newcard) (cellspace ?cells) (successor ?ncells ?cells))
  :effect (and (cellspace ?ncells) (clear ?card) (on ?card ?newcard) (not (incell ?card)) (not (cellspace ?cells)) (not (clear ?newcard))))
 (:action newcolfromfreecell
  :parameters ( ?card - card ?cols - denomination ?ncols - denomination ?cells - denomination ?ncells - denomination)
  :precondition (and (incell ?card) (colspace ?cols) (cellspace ?cells) (successor ?cols ?ncols) (successor ?ncells ?cells))
  :effect (and (bottomcol ?card) (clear ?card) (colspace ?ncols) (cellspace ?ncells) (not (incell ?card)) (not (colspace ?cols)) (not (cellspace ?cells))))
)
