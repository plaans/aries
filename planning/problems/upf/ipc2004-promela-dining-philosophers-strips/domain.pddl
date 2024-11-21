(define (domain grounded_strips_instance-domain)
 (:requirements :strips)
 (:predicates (activate_philosopher_1_forks__pid_wfork) (activate_philosopher_0_forks__pid_wfork) (enabled_philosopher_1_forks__pid_wfork) (advance_tail_forks_1_) (queue_tail_msg_forks_1__fork) (enabled_philosopher_0_forks__pid_wfork) (advance_tail_forks_0_) (queue_tail_msg_forks_0__fork) (queue_msg_forks_1__qs_0_fork) (queue_head_msg_forks_1__fork) (queue_size_forks_1__one) (queue_msg_forks_0__qs_0_fork) (queue_head_msg_forks_0__fork) (queue_size_forks_0__one) (at_process_philosopher_0_state_6) (at_process_philosopher_1_state_6) (activate_philosopher_1_forks__pid_rfork) (activate_philosopher_0_forks__pid_rfork) (advance_head_forks_1_) (enabled_philosopher_1_forks__pid_rfork) (advance_head_forks_0_) (enabled_philosopher_0_forks__pid_rfork) (blocked_trans_philosopher_1_forks__pid_rfork) (blocked_trans_philosopher_0_forks__pid_rfork) (blocked_trans_philosopher_1_forks__pid_wfork) (blocked_trans_philosopher_0_forks__pid_wfork) (at_process_philosopher_0_state_3) (at_process_philosopher_1_state_3) (activate_philosopher_1_forks____pidp1__2__rfork) (activate_philosopher_0_forks____pidp1__2__rfork) (blocked_philosopher_0) (blocked_philosopher_1) (enabled_philosopher_1_forks____pidp1__2__rfork) (enabled_philosopher_0_forks____pidp1__2__rfork) (blocked_trans_philosopher_1_forks____pidp1__2__rfork) (blocked_trans_philosopher_0_forks____pidp1__2__rfork) (at_process_philosopher_0_state_4) (at_process_philosopher_1_state_4) (at_process_philosopher_0_state_5) (at_process_philosopher_1_state_5) (activate_philosopher_1_forks____pidp1__2__wfork) (activate_philosopher_0_forks____pidp1__2__wfork) (blocked_trans_philosopher_0_forks____pidp1__2__wfork) (blocked_trans_philosopher_1_forks____pidp1__2__wfork) (enabled_philosopher_1_forks____pidp1__2__wfork) (enabled_philosopher_0_forks____pidp1__2__wfork) (settled_forks_1_) (settled_forks_0_) (pending_philosopher_0) (pending_philosopher_1) (queue_head_forks_0__qs_0) (queue_head_forks_1__qs_0) (at_process_philosopher_1_state_1) (at_process_philosopher_0_state_1) (queue_tail_forks_0__qs_0) (queue_size_forks_0__zero) (queue_head_msg_forks_0__empty) (queue_tail_forks_1__qs_0) (queue_size_forks_1__zero) (queue_head_msg_forks_1__empty))
 (:action block_philosopher_1_state_5_philosopher_0
  :parameters ()
  :precondition (and (at_process_philosopher_1_state_5) (blocked_trans_philosopher_1_forks____pidp1__2__wfork))
  :effect (and (blocked_philosopher_1)))
 (:action block_philosopher_0_state_5_philosopher_0
  :parameters ()
  :precondition (and (at_process_philosopher_0_state_5) (blocked_trans_philosopher_0_forks____pidp1__2__wfork))
  :effect (and (blocked_philosopher_0)))
 (:action perform_trans_philosopher_1_philosopher_forks____pidp1__2__wfork_state_5_state_6_0
  :parameters ()
  :precondition (and (at_process_philosopher_1_state_5) (enabled_philosopher_1_forks____pidp1__2__wfork) (settled_forks_0_) (settled_forks_1_))
  :effect (and (at_process_philosopher_1_state_6) (pending_philosopher_1) (not (at_process_philosopher_1_state_5)) (not (enabled_philosopher_1_forks____pidp1__2__wfork))))
 (:action perform_trans_philosopher_0_philosopher_forks____pidp1__2__wfork_state_5_state_6_0
  :parameters ()
  :precondition (and (at_process_philosopher_0_state_5) (enabled_philosopher_0_forks____pidp1__2__wfork) (settled_forks_0_) (settled_forks_1_))
  :effect (and (at_process_philosopher_0_state_6) (pending_philosopher_0) (not (at_process_philosopher_0_state_5)) (not (enabled_philosopher_0_forks____pidp1__2__wfork))))
 (:action queue_write_philosopher_0_forks____pidp1__2__wfork_forks_1__fork_0
  :parameters ()
  :precondition (and (settled_forks_1_) (activate_philosopher_0_forks____pidp1__2__wfork))
  :effect (and (enabled_philosopher_0_forks____pidp1__2__wfork) (advance_tail_forks_1_) (queue_tail_msg_forks_1__fork) (not (blocked_trans_philosopher_0_forks__pid_wfork)) (not (blocked_trans_philosopher_0_forks__pid_rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__wfork)) (not (blocked_trans_philosopher_1_forks__pid_wfork)) (not (blocked_trans_philosopher_1_forks__pid_rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__wfork)) (not (blocked_philosopher_0)) (not (blocked_philosopher_1)) (not (settled_forks_1_)) (not (activate_philosopher_0_forks____pidp1__2__wfork))))
 (:action queue_write_philosopher_1_forks____pidp1__2__wfork_forks_0__fork_0
  :parameters ()
  :precondition (and (settled_forks_0_) (activate_philosopher_1_forks____pidp1__2__wfork))
  :effect (and (enabled_philosopher_1_forks____pidp1__2__wfork) (advance_tail_forks_0_) (queue_tail_msg_forks_0__fork) (not (blocked_trans_philosopher_0_forks__pid_wfork)) (not (blocked_trans_philosopher_0_forks__pid_rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__wfork)) (not (blocked_trans_philosopher_1_forks__pid_wfork)) (not (blocked_trans_philosopher_1_forks__pid_rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__wfork)) (not (blocked_philosopher_0)) (not (blocked_philosopher_1)) (not (settled_forks_0_)) (not (activate_philosopher_1_forks____pidp1__2__wfork))))
 (:action block_write_philosopher_1_forks____pidp1__2__wfork_forks_0__queue_1_fork_one_0
  :parameters ()
  :precondition (and (queue_size_forks_0__one) (settled_forks_0_) (activate_philosopher_1_forks____pidp1__2__wfork))
  :effect (and (blocked_trans_philosopher_1_forks____pidp1__2__wfork) (not (enabled_philosopher_1_forks____pidp1__2__wfork)) (not (activate_philosopher_1_forks____pidp1__2__wfork)) (not (settled_forks_0_))))
 (:action block_write_philosopher_0_forks____pidp1__2__wfork_forks_1__queue_1_fork_one_0
  :parameters ()
  :precondition (and (queue_size_forks_1__one) (settled_forks_1_) (activate_philosopher_0_forks____pidp1__2__wfork))
  :effect (and (blocked_trans_philosopher_0_forks____pidp1__2__wfork) (not (enabled_philosopher_0_forks____pidp1__2__wfork)) (not (activate_philosopher_0_forks____pidp1__2__wfork)) (not (settled_forks_1_))))
 (:action activate_trans_philosopher_0_philosopher_forks____pidp1__2__wfork_state_5_state_6_0
  :parameters ()
  :precondition (and (pending_philosopher_0) (at_process_philosopher_0_state_5) (settled_forks_0_) (settled_forks_1_))
  :effect (and (activate_philosopher_0_forks____pidp1__2__wfork) (not (pending_philosopher_0))))
 (:action activate_trans_philosopher_1_philosopher_forks____pidp1__2__wfork_state_5_state_6_0
  :parameters ()
  :precondition (and (pending_philosopher_1) (at_process_philosopher_1_state_5) (settled_forks_0_) (settled_forks_1_))
  :effect (and (activate_philosopher_1_forks____pidp1__2__wfork) (not (pending_philosopher_1))))
 (:action perform_trans_philosopher_1_philosopher_forks__pid_wfork_state_4_state_5_0
  :parameters ()
  :precondition (and (at_process_philosopher_1_state_4) (enabled_philosopher_1_forks__pid_wfork) (settled_forks_0_) (settled_forks_1_))
  :effect (and (at_process_philosopher_1_state_5) (pending_philosopher_1) (not (at_process_philosopher_1_state_4)) (not (enabled_philosopher_1_forks__pid_wfork))))
 (:action perform_trans_philosopher_0_philosopher_forks__pid_wfork_state_4_state_5_0
  :parameters ()
  :precondition (and (at_process_philosopher_0_state_4) (enabled_philosopher_0_forks__pid_wfork) (settled_forks_0_) (settled_forks_1_))
  :effect (and (at_process_philosopher_0_state_5) (pending_philosopher_0) (not (at_process_philosopher_0_state_4)) (not (enabled_philosopher_0_forks__pid_wfork))))
 (:action block_philosopher_1_state_4_philosopher_0
  :parameters ()
  :precondition (and (at_process_philosopher_1_state_4) (blocked_trans_philosopher_1_forks__pid_wfork))
  :effect (and (blocked_philosopher_1)))
 (:action block_philosopher_1_state_3_philosopher_0
  :parameters ()
  :precondition (and (at_process_philosopher_1_state_3) (blocked_trans_philosopher_1_forks____pidp1__2__rfork))
  :effect (and (blocked_philosopher_1)))
 (:action block_philosopher_0_state_4_philosopher_0
  :parameters ()
  :precondition (and (at_process_philosopher_0_state_4) (blocked_trans_philosopher_0_forks__pid_wfork))
  :effect (and (blocked_philosopher_0)))
 (:action block_philosopher_0_state_3_philosopher_0
  :parameters ()
  :precondition (and (at_process_philosopher_0_state_3) (blocked_trans_philosopher_0_forks____pidp1__2__rfork))
  :effect (and (blocked_philosopher_0)))
 (:action activate_trans_philosopher_0_philosopher_forks__pid_wfork_state_4_state_5_0
  :parameters ()
  :precondition (and (pending_philosopher_0) (at_process_philosopher_0_state_4) (settled_forks_0_) (settled_forks_1_))
  :effect (and (activate_philosopher_0_forks__pid_wfork) (not (pending_philosopher_0))))
 (:action activate_trans_philosopher_1_philosopher_forks__pid_wfork_state_4_state_5_0
  :parameters ()
  :precondition (and (pending_philosopher_1) (at_process_philosopher_1_state_4) (settled_forks_0_) (settled_forks_1_))
  :effect (and (activate_philosopher_1_forks__pid_wfork) (not (pending_philosopher_1))))
 (:action perform_trans_philosopher_1_philosopher_forks____pidp1__2__rfork_state_3_state_4_0
  :parameters ()
  :precondition (and (at_process_philosopher_1_state_3) (enabled_philosopher_1_forks____pidp1__2__rfork) (settled_forks_0_) (settled_forks_1_))
  :effect (and (at_process_philosopher_1_state_4) (pending_philosopher_1) (not (at_process_philosopher_1_state_3)) (not (enabled_philosopher_1_forks____pidp1__2__rfork))))
 (:action perform_trans_philosopher_0_philosopher_forks____pidp1__2__rfork_state_3_state_4_0
  :parameters ()
  :precondition (and (at_process_philosopher_0_state_3) (enabled_philosopher_0_forks____pidp1__2__rfork) (settled_forks_0_) (settled_forks_1_))
  :effect (and (at_process_philosopher_0_state_4) (pending_philosopher_0) (not (at_process_philosopher_0_state_3)) (not (enabled_philosopher_0_forks____pidp1__2__rfork))))
 (:action block_read_wrong_message_philosopher_0_forks____pidp1__2__rfork_forks_1__fork_empty_0
  :parameters ()
  :precondition (and (queue_head_msg_forks_1__empty) (settled_forks_1_) (activate_philosopher_0_forks____pidp1__2__rfork))
  :effect (and (blocked_trans_philosopher_0_forks____pidp1__2__rfork) (not (enabled_philosopher_0_forks____pidp1__2__rfork)) (not (activate_philosopher_0_forks____pidp1__2__rfork)) (not (settled_forks_1_))))
 (:action block_read_wrong_message_philosopher_1_forks____pidp1__2__rfork_forks_0__fork_empty_0
  :parameters ()
  :precondition (and (queue_head_msg_forks_0__empty) (settled_forks_0_) (activate_philosopher_1_forks____pidp1__2__rfork))
  :effect (and (blocked_trans_philosopher_1_forks____pidp1__2__rfork) (not (enabled_philosopher_1_forks____pidp1__2__rfork)) (not (activate_philosopher_1_forks____pidp1__2__rfork)) (not (settled_forks_0_))))
 (:action block_read_queue_empty_philosopher_0_forks____pidp1__2__rfork_forks_1__fork_zero_0
  :parameters ()
  :precondition (and (queue_size_forks_1__zero) (settled_forks_1_) (activate_philosopher_0_forks____pidp1__2__rfork))
  :effect (and (blocked_trans_philosopher_0_forks____pidp1__2__rfork) (not (enabled_philosopher_0_forks____pidp1__2__rfork)) (not (activate_philosopher_0_forks____pidp1__2__rfork)) (not (settled_forks_1_))))
 (:action block_read_queue_empty_philosopher_1_forks____pidp1__2__rfork_forks_0__fork_zero_0
  :parameters ()
  :precondition (and (queue_size_forks_0__zero) (settled_forks_0_) (activate_philosopher_1_forks____pidp1__2__rfork))
  :effect (and (blocked_trans_philosopher_1_forks____pidp1__2__rfork) (not (enabled_philosopher_1_forks____pidp1__2__rfork)) (not (activate_philosopher_1_forks____pidp1__2__rfork)) (not (settled_forks_0_))))
 (:action queue_read_philosopher_0_forks____pidp1__2__rfork_forks_1__fork_0
  :parameters ()
  :precondition (and (queue_head_msg_forks_1__fork) (settled_forks_1_) (activate_philosopher_0_forks____pidp1__2__rfork))
  :effect (and (advance_head_forks_1_) (enabled_philosopher_0_forks____pidp1__2__rfork) (not (blocked_trans_philosopher_0_forks__pid_wfork)) (not (blocked_trans_philosopher_0_forks__pid_rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__wfork)) (not (blocked_trans_philosopher_1_forks__pid_wfork)) (not (blocked_trans_philosopher_1_forks__pid_rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__wfork)) (not (blocked_philosopher_0)) (not (blocked_philosopher_1)) (not (activate_philosopher_0_forks____pidp1__2__rfork)) (not (settled_forks_1_))))
 (:action queue_read_philosopher_1_forks____pidp1__2__rfork_forks_0__fork_0
  :parameters ()
  :precondition (and (queue_head_msg_forks_0__fork) (settled_forks_0_) (activate_philosopher_1_forks____pidp1__2__rfork))
  :effect (and (advance_head_forks_0_) (enabled_philosopher_1_forks____pidp1__2__rfork) (not (blocked_trans_philosopher_0_forks__pid_wfork)) (not (blocked_trans_philosopher_0_forks__pid_rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__wfork)) (not (blocked_trans_philosopher_1_forks__pid_wfork)) (not (blocked_trans_philosopher_1_forks__pid_rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__wfork)) (not (blocked_philosopher_0)) (not (blocked_philosopher_1)) (not (activate_philosopher_1_forks____pidp1__2__rfork)) (not (settled_forks_0_))))
 (:action block_philosopher_1_state_6_philosopher_0
  :parameters ()
  :precondition (and (at_process_philosopher_1_state_6) (blocked_trans_philosopher_1_forks__pid_rfork))
  :effect (and (blocked_philosopher_1)))
 (:action block_philosopher_1_state_1_philosopher_0
  :parameters ()
  :precondition (and (at_process_philosopher_1_state_1) (blocked_trans_philosopher_1_forks__pid_wfork))
  :effect (and (blocked_philosopher_1)))
 (:action block_philosopher_0_state_6_philosopher_0
  :parameters ()
  :precondition (and (at_process_philosopher_0_state_6) (blocked_trans_philosopher_0_forks__pid_rfork))
  :effect (and (blocked_philosopher_0)))
 (:action block_philosopher_0_state_1_philosopher_0
  :parameters ()
  :precondition (and (at_process_philosopher_0_state_1) (blocked_trans_philosopher_0_forks__pid_wfork))
  :effect (and (blocked_philosopher_0)))
 (:action activate_trans_philosopher_0_philosopher_forks____pidp1__2__rfork_state_3_state_4_0
  :parameters ()
  :precondition (and (pending_philosopher_0) (at_process_philosopher_0_state_3) (settled_forks_0_) (settled_forks_1_))
  :effect (and (activate_philosopher_0_forks____pidp1__2__rfork) (not (pending_philosopher_0))))
 (:action activate_trans_philosopher_1_philosopher_forks____pidp1__2__rfork_state_3_state_4_0
  :parameters ()
  :precondition (and (pending_philosopher_1) (at_process_philosopher_1_state_3) (settled_forks_0_) (settled_forks_1_))
  :effect (and (activate_philosopher_1_forks____pidp1__2__rfork) (not (pending_philosopher_1))))
 (:action perform_trans_philosopher_1_philosopher_forks__pid_rfork_state_6_state_3_0
  :parameters ()
  :precondition (and (at_process_philosopher_1_state_6) (enabled_philosopher_1_forks__pid_rfork) (settled_forks_0_) (settled_forks_1_))
  :effect (and (at_process_philosopher_1_state_3) (pending_philosopher_1) (not (at_process_philosopher_1_state_6)) (not (enabled_philosopher_1_forks__pid_rfork))))
 (:action perform_trans_philosopher_0_philosopher_forks__pid_rfork_state_6_state_3_0
  :parameters ()
  :precondition (and (at_process_philosopher_0_state_6) (enabled_philosopher_0_forks__pid_rfork) (settled_forks_0_) (settled_forks_1_))
  :effect (and (at_process_philosopher_0_state_3) (pending_philosopher_0) (not (at_process_philosopher_0_state_6)) (not (enabled_philosopher_0_forks__pid_rfork))))
 (:action advance_queue_head_forks_0__queue_1_qs_0_qs_0_fork_one_zero_0
  :parameters ()
  :precondition (and (queue_size_forks_0__one) (queue_msg_forks_0__qs_0_fork) (advance_head_forks_0_) (queue_head_forks_0__qs_0))
  :effect (and (settled_forks_0_) (queue_head_forks_0__qs_0) (queue_head_msg_forks_0__fork) (queue_size_forks_0__zero) (not (advance_head_forks_0_)) (not (queue_size_forks_0__one))))
 (:action advance_queue_head_forks_1__queue_1_qs_0_qs_0_fork_one_zero_0
  :parameters ()
  :precondition (and (queue_size_forks_1__one) (queue_msg_forks_1__qs_0_fork) (advance_head_forks_1_) (queue_head_forks_1__qs_0))
  :effect (and (settled_forks_1_) (queue_head_forks_1__qs_0) (queue_head_msg_forks_1__fork) (queue_size_forks_1__zero) (not (advance_head_forks_1_)) (not (queue_size_forks_1__one))))
 (:action block_read_wrong_message_philosopher_0_forks__pid_rfork_forks_0__fork_empty_0
  :parameters ()
  :precondition (and (queue_head_msg_forks_0__empty) (settled_forks_0_) (activate_philosopher_0_forks__pid_rfork))
  :effect (and (blocked_trans_philosopher_0_forks__pid_rfork) (not (enabled_philosopher_0_forks__pid_rfork)) (not (activate_philosopher_0_forks__pid_rfork)) (not (settled_forks_0_))))
 (:action block_read_wrong_message_philosopher_1_forks__pid_rfork_forks_1__fork_empty_0
  :parameters ()
  :precondition (and (queue_head_msg_forks_1__empty) (settled_forks_1_) (activate_philosopher_1_forks__pid_rfork))
  :effect (and (blocked_trans_philosopher_1_forks__pid_rfork) (not (enabled_philosopher_1_forks__pid_rfork)) (not (activate_philosopher_1_forks__pid_rfork)) (not (settled_forks_1_))))
 (:action block_write_philosopher_0_forks__pid_wfork_forks_0__queue_1_fork_one_0
  :parameters ()
  :precondition (and (queue_size_forks_0__one) (settled_forks_0_) (activate_philosopher_0_forks__pid_wfork))
  :effect (and (blocked_trans_philosopher_0_forks__pid_wfork) (not (enabled_philosopher_0_forks__pid_wfork)) (not (activate_philosopher_0_forks__pid_wfork)) (not (settled_forks_0_))))
 (:action block_write_philosopher_1_forks__pid_wfork_forks_1__queue_1_fork_one_0
  :parameters ()
  :precondition (and (queue_size_forks_1__one) (settled_forks_1_) (activate_philosopher_1_forks__pid_wfork))
  :effect (and (blocked_trans_philosopher_1_forks__pid_wfork) (not (enabled_philosopher_1_forks__pid_wfork)) (not (activate_philosopher_1_forks__pid_wfork)) (not (settled_forks_1_))))
 (:action block_read_queue_empty_philosopher_0_forks__pid_rfork_forks_0__fork_zero_0
  :parameters ()
  :precondition (and (queue_size_forks_0__zero) (settled_forks_0_) (activate_philosopher_0_forks__pid_rfork))
  :effect (and (blocked_trans_philosopher_0_forks__pid_rfork) (not (enabled_philosopher_0_forks__pid_rfork)) (not (activate_philosopher_0_forks__pid_rfork)) (not (settled_forks_0_))))
 (:action block_read_queue_empty_philosopher_1_forks__pid_rfork_forks_1__fork_zero_0
  :parameters ()
  :precondition (and (queue_size_forks_1__zero) (settled_forks_1_) (activate_philosopher_1_forks__pid_rfork))
  :effect (and (blocked_trans_philosopher_1_forks__pid_rfork) (not (enabled_philosopher_1_forks__pid_rfork)) (not (activate_philosopher_1_forks__pid_rfork)) (not (settled_forks_1_))))
 (:action queue_read_philosopher_0_forks__pid_rfork_forks_0__fork_0
  :parameters ()
  :precondition (and (queue_head_msg_forks_0__fork) (settled_forks_0_) (activate_philosopher_0_forks__pid_rfork))
  :effect (and (advance_head_forks_0_) (enabled_philosopher_0_forks__pid_rfork) (not (blocked_trans_philosopher_0_forks__pid_wfork)) (not (blocked_trans_philosopher_0_forks__pid_rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__wfork)) (not (blocked_trans_philosopher_1_forks__pid_wfork)) (not (blocked_trans_philosopher_1_forks__pid_rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__wfork)) (not (blocked_philosopher_0)) (not (blocked_philosopher_1)) (not (activate_philosopher_0_forks__pid_rfork)) (not (settled_forks_0_))))
 (:action queue_read_philosopher_1_forks__pid_rfork_forks_1__fork_0
  :parameters ()
  :precondition (and (queue_head_msg_forks_1__fork) (settled_forks_1_) (activate_philosopher_1_forks__pid_rfork))
  :effect (and (advance_head_forks_1_) (enabled_philosopher_1_forks__pid_rfork) (not (blocked_trans_philosopher_0_forks__pid_wfork)) (not (blocked_trans_philosopher_0_forks__pid_rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__wfork)) (not (blocked_trans_philosopher_1_forks__pid_wfork)) (not (blocked_trans_philosopher_1_forks__pid_rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__wfork)) (not (blocked_philosopher_0)) (not (blocked_philosopher_1)) (not (activate_philosopher_1_forks__pid_rfork)) (not (settled_forks_1_))))
 (:action activate_trans_philosopher_0_philosopher_forks__pid_rfork_state_6_state_3_0
  :parameters ()
  :precondition (and (pending_philosopher_0) (at_process_philosopher_0_state_6) (settled_forks_0_) (settled_forks_1_))
  :effect (and (activate_philosopher_0_forks__pid_rfork) (not (pending_philosopher_0))))
 (:action activate_trans_philosopher_1_philosopher_forks__pid_rfork_state_6_state_3_0
  :parameters ()
  :precondition (and (pending_philosopher_1) (at_process_philosopher_1_state_6) (settled_forks_0_) (settled_forks_1_))
  :effect (and (activate_philosopher_1_forks__pid_rfork) (not (pending_philosopher_1))))
 (:action perform_trans_philosopher_1_philosopher_forks__pid_wfork_state_1_state_6_0
  :parameters ()
  :precondition (and (at_process_philosopher_1_state_1) (enabled_philosopher_1_forks__pid_wfork) (settled_forks_0_) (settled_forks_1_))
  :effect (and (at_process_philosopher_1_state_6) (pending_philosopher_1) (not (at_process_philosopher_1_state_1)) (not (enabled_philosopher_1_forks__pid_wfork))))
 (:action perform_trans_philosopher_0_philosopher_forks__pid_wfork_state_1_state_6_0
  :parameters ()
  :precondition (and (at_process_philosopher_0_state_1) (enabled_philosopher_0_forks__pid_wfork) (settled_forks_0_) (settled_forks_1_))
  :effect (and (at_process_philosopher_0_state_6) (pending_philosopher_0) (not (at_process_philosopher_0_state_1)) (not (enabled_philosopher_0_forks__pid_wfork))))
 (:action advance_empty_queue_tail_forks_0__queue_1_qs_0_qs_0_fork_fork_zero_one_0
  :parameters ()
  :precondition (and (queue_size_forks_0__zero) (queue_head_msg_forks_0__fork) (queue_tail_msg_forks_0__fork) (advance_tail_forks_0_) (queue_tail_forks_0__qs_0))
  :effect (and (settled_forks_0_) (queue_tail_forks_0__qs_0) (queue_msg_forks_0__qs_0_fork) (queue_head_msg_forks_0__fork) (queue_size_forks_0__one) (not (advance_tail_forks_0_)) (not (queue_size_forks_0__zero))))
 (:action advance_empty_queue_tail_forks_0__queue_1_qs_0_qs_0_fork_empty_zero_one_0
  :parameters ()
  :precondition (and (queue_size_forks_0__zero) (queue_head_msg_forks_0__empty) (queue_tail_msg_forks_0__fork) (advance_tail_forks_0_) (queue_tail_forks_0__qs_0))
  :effect (and (settled_forks_0_) (queue_tail_forks_0__qs_0) (queue_msg_forks_0__qs_0_fork) (queue_head_msg_forks_0__fork) (queue_size_forks_0__one) (not (advance_tail_forks_0_)) (not (queue_head_msg_forks_0__empty)) (not (queue_size_forks_0__zero))))
 (:action advance_empty_queue_tail_forks_1__queue_1_qs_0_qs_0_fork_fork_zero_one_0
  :parameters ()
  :precondition (and (queue_size_forks_1__zero) (queue_head_msg_forks_1__fork) (queue_tail_msg_forks_1__fork) (advance_tail_forks_1_) (queue_tail_forks_1__qs_0))
  :effect (and (settled_forks_1_) (queue_tail_forks_1__qs_0) (queue_msg_forks_1__qs_0_fork) (queue_head_msg_forks_1__fork) (queue_size_forks_1__one) (not (advance_tail_forks_1_)) (not (queue_size_forks_1__zero))))
 (:action advance_empty_queue_tail_forks_1__queue_1_qs_0_qs_0_fork_empty_zero_one_0
  :parameters ()
  :precondition (and (queue_size_forks_1__zero) (queue_head_msg_forks_1__empty) (queue_tail_msg_forks_1__fork) (advance_tail_forks_1_) (queue_tail_forks_1__qs_0))
  :effect (and (settled_forks_1_) (queue_tail_forks_1__qs_0) (queue_msg_forks_1__qs_0_fork) (queue_head_msg_forks_1__fork) (queue_size_forks_1__one) (not (advance_tail_forks_1_)) (not (queue_head_msg_forks_1__empty)) (not (queue_size_forks_1__zero))))
 (:action queue_write_philosopher_0_forks__pid_wfork_forks_0__fork_0
  :parameters ()
  :precondition (and (settled_forks_0_) (activate_philosopher_0_forks__pid_wfork))
  :effect (and (enabled_philosopher_0_forks__pid_wfork) (advance_tail_forks_0_) (queue_tail_msg_forks_0__fork) (not (blocked_trans_philosopher_0_forks__pid_wfork)) (not (blocked_trans_philosopher_0_forks__pid_rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__wfork)) (not (blocked_trans_philosopher_1_forks__pid_wfork)) (not (blocked_trans_philosopher_1_forks__pid_rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__wfork)) (not (blocked_philosopher_0)) (not (blocked_philosopher_1)) (not (settled_forks_0_)) (not (activate_philosopher_0_forks__pid_wfork))))
 (:action queue_write_philosopher_1_forks__pid_wfork_forks_1__fork_0
  :parameters ()
  :precondition (and (settled_forks_1_) (activate_philosopher_1_forks__pid_wfork))
  :effect (and (enabled_philosopher_1_forks__pid_wfork) (advance_tail_forks_1_) (queue_tail_msg_forks_1__fork) (not (blocked_trans_philosopher_0_forks__pid_wfork)) (not (blocked_trans_philosopher_0_forks__pid_rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_0_forks____pidp1__2__wfork)) (not (blocked_trans_philosopher_1_forks__pid_wfork)) (not (blocked_trans_philosopher_1_forks__pid_rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__rfork)) (not (blocked_trans_philosopher_1_forks____pidp1__2__wfork)) (not (blocked_philosopher_0)) (not (blocked_philosopher_1)) (not (settled_forks_1_)) (not (activate_philosopher_1_forks__pid_wfork))))
 (:action activate_trans_philosopher_0_philosopher_forks__pid_wfork_state_1_state_6_0
  :parameters ()
  :precondition (and (pending_philosopher_0) (at_process_philosopher_0_state_1) (settled_forks_0_) (settled_forks_1_))
  :effect (and (activate_philosopher_0_forks__pid_wfork) (not (pending_philosopher_0))))
 (:action activate_trans_philosopher_1_philosopher_forks__pid_wfork_state_1_state_6_0
  :parameters ()
  :precondition (and (pending_philosopher_1) (at_process_philosopher_1_state_1) (settled_forks_0_) (settled_forks_1_))
  :effect (and (activate_philosopher_1_forks__pid_wfork) (not (pending_philosopher_1))))
)
