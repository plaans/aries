(define (domain generischeslinearesverkabelungsproblemtiefe1-domain)
 (:requirements :strips :typing :negative-preconditions :hierarchy :method-preconditions)
 (:types
    port abstractdevice enum - object
    plugtype plugface plugdirection signaltype - enum
    abstractcable device - abstractdevice
    cable adapter - abstractcable
 )
 (:constants
   male female - plugface
   both out in - plugdirection
 )
 (:predicates (isplugtype ?p - port ?t - plugtype) (isplugface ?p - port ?f - plugface) (isplugdirection ?p - port ?d - plugdirection) (issignalsource ?p - port ?t_0 - signaltype) (issignaldestination ?p - port ?t_0 - signaltype) (issignalrepeater ?p1 - port ?p2 - port ?t_0 - signaltype) (ispartof ?p - port ?d_0 - abstractdevice) (isconnected ?p1 - port ?p2 - port) (isoccupied ?p - port) (pguard) (paim))
 (:task connectdevices
  :parameters ( ?d1 - abstractdevice ?d2 - abstractdevice ?t_0 - signaltype))
 (:task validatedeviceconnection
  :parameters ( ?d1 - abstractdevice ?d2 - abstractdevice ?t_0 - signaltype))
 (:task validateportconnection
  :parameters ( ?p1 - port ?p2 - port ?t_0 - signaltype))
 (:task connect
  :parameters ( ?p1 - port ?p2 - port))
 (:method c1
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :task (connect ?p1 ?p2)
  :ordered-subtasks (and
    (_t1 (connect_1 ?p1 ?p2 ?t))))
 (:method c2
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :task (connect ?p1 ?p2)
  :ordered-subtasks (and
    (_t2 (connect_2 ?p1 ?p2 ?t))))
 (:method c3
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :task (connect ?p1 ?p2)
  :ordered-subtasks (and
    (_t3 (connect_3 ?p1 ?p2 ?t))))
 (:method c4
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :task (connect ?p1 ?p2)
  :ordered-subtasks (and
    (_t4 (connect_4 ?p1 ?p2 ?t))))
 (:method c5
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :task (connect ?p1 ?p2)
  :ordered-subtasks (and
    (_t5 (connect_5 ?p1 ?p2 ?t))))
 (:method c6
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :task (connect ?p1 ?p2)
  :ordered-subtasks (and
    (_t6 (connect_6 ?p1 ?p2 ?t))))
 (:method c7
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :task (connect ?p1 ?p2)
  :ordered-subtasks (and
    (_t7 (connect_7 ?p1 ?p2 ?t))))
 (:method c8
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :task (connect ?p1 ?p2)
  :ordered-subtasks (and
    (_t8 (connect_8 ?p1 ?p2 ?t))))
 (:method m1
  :parameters ( ?d1 - abstractdevice ?d2 - abstractdevice ?t_0 - signaltype ?p1 - port ?p2 - port)
  :task (connectdevices ?d1 ?d2 ?t_0)
  :ordered-subtasks (and
    (t1 (connect ?p1 ?p2))
    (t2 (connectdevices ?d1 ?d2 ?t_0))))
 (:method m2
  :parameters ( ?d1 - abstractdevice ?d2 - abstractdevice ?t_0 - signaltype ?p1 - port ?p2 - port)
  :task (connectdevices ?d1 ?d2 ?t_0)
  :ordered-subtasks (and
    (t1 (disconnect ?p1 ?p2))
    (t2 (connectdevices ?d1 ?d2 ?t_0))))
 (:method m3
  :parameters ( ?d1 - abstractdevice ?d2 - abstractdevice ?t_0 - signaltype)
  :task (connectdevices ?d1 ?d2 ?t_0)
  :ordered-subtasks (and
    (t1 (guard ))
    (t2 (validatedeviceconnection ?d1 ?d2 ?t_0))))
 (:method vdc_to_vpc_1
  :parameters ( ?d1 - abstractdevice ?d2 - abstractdevice ?t_0 - signaltype ?p1 - port ?p2 - port)
  :task (validatedeviceconnection ?d1 ?d2 ?t_0)
  :precondition (and (ispartof ?p1 ?d1) (ispartof ?p2 ?d2) (isplugdirection ?p1 out) (isplugdirection ?p2 in) (issignalsource ?p1 ?t_0) (issignaldestination ?p2 ?t_0))
  :ordered-subtasks (and
    (_t15 (validateportconnection ?p1 ?p2 ?t_0))))
 (:method vdc_to_vpc_2
  :parameters ( ?d1 - abstractdevice ?d2 - abstractdevice ?t_0 - signaltype ?p1 - port ?p2 - port)
  :task (validatedeviceconnection ?d1 ?d2 ?t_0)
  :precondition (and (ispartof ?p1 ?d1) (ispartof ?p2 ?d2) (isplugdirection ?p1 out) (isplugdirection ?p2 both) (issignalsource ?p1 ?t_0) (issignaldestination ?p2 ?t_0))
  :ordered-subtasks (and
    (_t16 (validateportconnection ?p1 ?p2 ?t_0))))
 (:method vdc_to_vpc_3
  :parameters ( ?d1 - abstractdevice ?d2 - abstractdevice ?t_0 - signaltype ?p1 - port ?p2 - port)
  :task (validatedeviceconnection ?d1 ?d2 ?t_0)
  :precondition (and (ispartof ?p1 ?d1) (ispartof ?p2 ?d2) (isplugdirection ?p1 both) (isplugdirection ?p2 in) (issignalsource ?p1 ?t_0) (issignaldestination ?p2 ?t_0))
  :ordered-subtasks (and
    (_t17 (validateportconnection ?p1 ?p2 ?t_0))))
 (:method vdc_to_vpc_4
  :parameters ( ?d1 - abstractdevice ?d2 - abstractdevice ?t_0 - signaltype ?p1 - port ?p2 - port)
  :task (validatedeviceconnection ?d1 ?d2 ?t_0)
  :precondition (and (ispartof ?p1 ?d1) (ispartof ?p2 ?d2) (isplugdirection ?p1 both) (isplugdirection ?p2 both) (issignalsource ?p1 ?t_0) (issignaldestination ?p2 ?t_0))
  :ordered-subtasks (and
    (_t18 (validateportconnection ?p1 ?p2 ?t_0))))
 (:method vpc_to_vpc
  :parameters ( ?t_0 - signaltype ?p1 - port ?p2 - port ?p3 - port ?p4 - port)
  :task (validateportconnection ?p1 ?p2 ?t_0)
  :precondition (and (isconnected ?p1 ?p3) (issignalrepeater ?p3 ?p4 ?t_0))
  :ordered-subtasks (and
    (_t19 (validateportconnection ?p4 ?p2 ?t_0))))
 (:method finish
  :parameters ( ?t_0 - signaltype ?p1 - port ?p2 - port)
  :task (validateportconnection ?p1 ?p2 ?t_0)
  :precondition (and (isconnected ?p1 ?p2))
  :ordered-subtasks (and
    (_t20 (ok ))))
 (:action connect_1
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :precondition (and (not (pguard)) (not (isoccupied ?p1)) (not (isoccupied ?p2)) (isplugtype ?p1 ?t) (isplugtype ?p2 ?t) (isplugface ?p1 male) (isplugface ?p2 female) (isplugdirection ?p1 out) (isplugdirection ?p2 in))
  :effect (and (isconnected ?p1 ?p2) (isoccupied ?p1) (isoccupied ?p2)))
 (:action connect_2
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :precondition (and (not (pguard)) (not (isoccupied ?p1)) (not (isoccupied ?p2)) (isplugtype ?p1 ?t) (isplugtype ?p2 ?t) (isplugface ?p1 male) (isplugface ?p2 female) (isplugdirection ?p1 in) (isplugdirection ?p2 out))
  :effect (and (isconnected ?p1 ?p2) (isoccupied ?p1) (isoccupied ?p2)))
 (:action connect_3
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :precondition (and (not (pguard)) (not (isoccupied ?p1)) (not (isoccupied ?p2)) (isplugtype ?p1 ?t) (isplugtype ?p2 ?t) (isplugface ?p1 male) (isplugface ?p2 female) (isplugdirection ?p1 both))
  :effect (and (isconnected ?p1 ?p2) (isoccupied ?p1) (isoccupied ?p2)))
 (:action connect_4
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :precondition (and (not (pguard)) (not (isoccupied ?p1)) (not (isoccupied ?p2)) (isplugtype ?p1 ?t) (isplugtype ?p2 ?t) (isplugface ?p1 male) (isplugface ?p2 female) (isplugdirection ?p2 both))
  :effect (and (isconnected ?p1 ?p2) (isoccupied ?p1) (isoccupied ?p2)))
 (:action connect_5
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :precondition (and (not (pguard)) (not (isoccupied ?p1)) (not (isoccupied ?p2)) (isplugtype ?p1 ?t) (isplugtype ?p2 ?t) (isplugface ?p1 female) (isplugface ?p2 male) (isplugdirection ?p1 out) (isplugdirection ?p2 in))
  :effect (and (isconnected ?p1 ?p2) (isoccupied ?p1) (isoccupied ?p2)))
 (:action connect_6
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :precondition (and (not (pguard)) (not (isoccupied ?p1)) (not (isoccupied ?p2)) (isplugtype ?p1 ?t) (isplugtype ?p2 ?t) (isplugface ?p1 female) (isplugface ?p2 male) (isplugdirection ?p1 in) (isplugdirection ?p2 out))
  :effect (and (isconnected ?p1 ?p2) (isoccupied ?p1) (isoccupied ?p2)))
 (:action connect_7
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :precondition (and (not (pguard)) (not (isoccupied ?p1)) (not (isoccupied ?p2)) (isplugtype ?p1 ?t) (isplugtype ?p2 ?t) (isplugface ?p1 female) (isplugface ?p2 male) (isplugdirection ?p1 both))
  :effect (and (isconnected ?p1 ?p2) (isoccupied ?p1) (isoccupied ?p2)))
 (:action connect_8
  :parameters ( ?p1 - port ?p2 - port ?t - plugtype)
  :precondition (and (not (pguard)) (not (isoccupied ?p1)) (not (isoccupied ?p2)) (isplugtype ?p1 ?t) (isplugtype ?p2 ?t) (isplugface ?p1 female) (isplugface ?p2 male) (isplugdirection ?p2 both))
  :effect (and (isconnected ?p1 ?p2) (isoccupied ?p1) (isoccupied ?p2)))
 (:action disconnect
  :parameters ( ?p1 - port ?p2 - port)
  :precondition (and (not (pguard)) (isconnected ?p1 ?p2))
  :effect (and (not (isconnected ?p1 ?p2)) (not (isoccupied ?p1)) (not (isoccupied ?p2))))
 (:action guard
  :parameters ()
  :effect (and (pguard)))
 (:action ok
  :parameters ()
  :precondition (and (pguard))
  :effect (and (paim)))
)
