(define (problem generischeslinearesverkabelungsproblemtiefe1-problem)
 (:domain generischeslinearesverkabelungsproblemtiefe1-domain)
 (:objects
   cablewithplugtype1_a cablewithplugtype1_b printer_aplugtype1 pc_bplugtype1 - port
   pc printer - device
   cablewithplugtype1 - cable
   plugtype1 - plugtype
   data - signaltype
 )
 (:htn
  :ordered-subtasks (and
    (_t21 (connectdevices pc printer data))))
 (:init (ispartof pc_bplugtype1 pc) (isplugtype pc_bplugtype1 plugtype1) (isplugface pc_bplugtype1 female) (isplugdirection pc_bplugtype1 out) (issignalsource pc_bplugtype1 data) (ispartof printer_aplugtype1 printer) (isplugtype printer_aplugtype1 plugtype1) (isplugface printer_aplugtype1 female) (isplugdirection printer_aplugtype1 in) (issignaldestination printer_aplugtype1 data) (ispartof cablewithplugtype1_a cablewithplugtype1) (ispartof cablewithplugtype1_b cablewithplugtype1) (isplugtype cablewithplugtype1_a plugtype1) (isplugtype cablewithplugtype1_b plugtype1) (isplugface cablewithplugtype1_a male) (isplugface cablewithplugtype1_b male) (isplugdirection cablewithplugtype1_a both) (isplugdirection cablewithplugtype1_b both) (issignalrepeater cablewithplugtype1_a cablewithplugtype1_b data) (issignalrepeater cablewithplugtype1_b cablewithplugtype1_a data))
 (:goal (and (paim)))
)
