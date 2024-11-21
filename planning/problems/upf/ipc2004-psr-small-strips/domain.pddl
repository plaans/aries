(define (domain grounded_strips_psr_s2_n1_l2_f50-domain)
 (:requirements :strips)
 (:predicates (not_closed_cb1) (updated_cb1) (not_closed_sd1) (not_closed_sd2) (closed_cb1) (closed_sd2) (closed_sd1) (not_updated_cb1) (do_close_sd1_condeffs) (do_wait_cb1_condeffs) (do_normal) (done_0))
 (:action close_sd2
  :parameters ()
  :precondition (and (do_normal) (not_closed_sd2) (updated_cb1))
  :effect (and (closed_sd2) (not (not_closed_sd2))))
 (:action close_sd1
  :parameters ()
  :precondition (and (do_normal) (not_closed_sd1) (updated_cb1))
  :effect (and (not (do_normal)) (do_close_sd1_condeffs) (closed_sd1) (not (not_closed_sd1))))
 (:action close_sd1_condeff0_yes
  :parameters ()
  :precondition (and (do_close_sd1_condeffs) (closed_cb1))
  :effect (and (done_0) (not_closed_cb1) (not (closed_cb1))))
 (:action close_sd1_condeff0_no_0
  :parameters ()
  :precondition (and (do_close_sd1_condeffs) (not_closed_cb1))
  :effect (and (done_0)))
 (:action close_sd1_endof_condeffs
  :parameters ()
  :precondition (and (do_close_sd1_condeffs) (done_0))
  :effect (and (do_normal) (not (do_close_sd1_condeffs)) (not (done_0))))
 (:action close_cb1
  :parameters ()
  :precondition (and (do_normal) (not_closed_cb1) (updated_cb1))
  :effect (and (closed_cb1) (not_updated_cb1) (not (not_closed_cb1)) (not (updated_cb1))))
 (:action open_sd2
  :parameters ()
  :precondition (and (do_normal) (closed_sd2) (updated_cb1))
  :effect (and (not_closed_sd2) (not (closed_sd2))))
 (:action open_sd1
  :parameters ()
  :precondition (and (do_normal) (closed_sd1) (updated_cb1))
  :effect (and (not_closed_sd1) (not (closed_sd1))))
 (:action open_cb1
  :parameters ()
  :precondition (and (do_normal) (closed_cb1) (updated_cb1))
  :effect (and (not_closed_cb1) (not (closed_cb1))))
 (:action wait_cb1
  :parameters ()
  :precondition (and (do_normal) (not_updated_cb1))
  :effect (and (not (do_normal)) (do_wait_cb1_condeffs) (updated_cb1) (not (not_updated_cb1))))
 (:action wait_cb1_condeff0_yes
  :parameters ()
  :precondition (and (do_wait_cb1_condeffs) (closed_sd1))
  :effect (and (done_0) (not_closed_cb1) (not (closed_cb1))))
 (:action wait_cb1_condeff0_no_0
  :parameters ()
  :precondition (and (do_wait_cb1_condeffs) (not_closed_sd1))
  :effect (and (done_0)))
 (:action wait_cb1_endof_condeffs
  :parameters ()
  :precondition (and (do_wait_cb1_condeffs) (done_0))
  :effect (and (do_normal) (not (do_wait_cb1_condeffs)) (not (done_0))))
)
