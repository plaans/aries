#[cfg(test)]
mod test {

    use crate::utils::*;
    use crate::utils::{actions::*, instance::*, states::*};
    use aries_solver::lang::Var;
    use aries_solver::{core::INT_CST_MIN, lang::Lit};
    use aries_timelines::{Effect, EffectOp, StateVar, explain::ExplainableSolver, symbols::ObjectEncoding};
    use aries_timelines::{Sched, Task};

    fn instance_1() -> Instance {
        Instance {
            jig_types: vec![
                JigType {
                    name: String::from("typeA"),
                    size_empty: 4,
                    size_loaded: 6,
                },
                JigType {
                    name: String::from("typeB"),
                    size_empty: 8,
                    size_loaded: 11,
                },
            ],
            jigs: vec![
                Jig {
                    name: String::from("jig001"),
                    jig_type: 0,
                    empty: true,
                },
                Jig {
                    name: String::from("jig002"),
                    jig_type: 1,
                    empty: false,
                },
                Jig {
                    name: String::from("jig003"),
                    jig_type: 0,
                    empty: false,
                },
                Jig {
                    name: String::from("jig004"),
                    jig_type: 0,
                    empty: false,
                },
            ],
            trailers_beluga: vec![Trailer {
                name: "trailer_beluga_0".to_string(),
                side: Side::Beluga,
            }],
            trailers_factory: vec![Trailer {
                name: "trailer_factory_0".to_string(),
                side: Side::Factory,
            }],
            hangars: vec!["hangar0".to_string(), "hangar1".to_string()],
            racks: vec![
                Rack {
                    name: "rack0".to_string(),
                    size: 32,
                    jigs: vec![0],
                },
                Rack {
                    name: "rack0".to_string(),
                    size: 32,
                    jigs: vec![],
                },
            ],
            production_lines: vec![ProductionLine {
                name: "pl0".to_string(),
                schedule: vec![1],
            }],
            flights: vec![
                Flight {
                    name: "beluga0".to_string(),
                    incoming: vec![1],
                    outgoing: vec![],
                },
                Flight {
                    name: "beluga0".to_string(),
                    incoming: vec![2, 3],
                    outgoing: vec![],
                },
            ],
        }
    }

    //Test mod instance
    #[test]
    fn test_bounds() {
        assert_eq!(instance_1().bounds_incoming(), (0, 1));
        assert_eq!(instance_1().bounds_outgoing(), (0, -1));
        assert_eq!(instance_1().bounds_jig_holder(), (0, 1));
    }

    #[test]
    fn test_size_of_jig() {
        assert_eq!(4, instance_1().size_of_jig(0, true).unwrap());
        assert_eq!(11, instance_1().size_of_jig(1, false).unwrap());
        assert_eq!(6, instance_1().size_of_jig(2, false).unwrap());
        assert_eq!(6, instance_1().size_of_jig(3, false).unwrap());
    }

    //Test mod states

    fn assign_and_assert_state_var(state_var: StateVar, value: i32) {
        let objects = ObjectEncoding::empty();
        let mut model = aries_timelines::Sched::new(1, objects);
        let mutex_end = model.new_timepoint();
        model.add_effect(Effect {
            transition_start: model.origin,
            transition_end: model.origin,
            mutex_end,
            state_var: state_var.clone(),
            operation: EffectOp::Assign(value.into()),
            prez: Lit::TRUE,
            source: None,
        });
        let param = ActParam {
            start: model.origin,
            end: model.origin,
            presence: Lit::TRUE,
            source: None,
        };
        let var_value = get_current_value(state_var, &mut model, &param);
        let mut solver: ExplainableSolver<()> = aries_timelines::explain::ExplainableSolver::new(&model, |_| None);
        if let Some(solution) = solver.check_satisfiability() {
            let sol_value = solution.eval(var_value).unwrap();
            assert_eq!(sol_value, value);
        }
    }

    #[test]
    fn test_get_current_value() {
        assign_and_assert_state_var(current_beluga(), 32);
        assign_and_assert_state_var(jig_loc(0, JigLocAttr::HeldBy), 5);
        assign_and_assert_state_var(jig_state(54, JigStateAttr::Size), 157);
        assign_and_assert_state_var(next_pos_in_incoming(), 5);
        assign_and_assert_state_var(next_pos_in_outgoing(), 0);
        assign_and_assert_state_var(last_pos_on_rack(4), -1);
        assign_and_assert_state_var(free_space_on_rack(4), 1000);
        assign_and_assert_state_var(
            StateVar {
                fluent: "".to_string(),
                args: vec![],
            },
            INT_CST_MIN,
        );
    }

    fn assert_var_value(model: &mut Sched, vars: Option<Vec<(Var, i32)>>) {
        let mut solver: ExplainableSolver<()> = aries_timelines::explain::ExplainableSolver::new(&model, |_| None);
        match solver.check_satisfiability() {
            Some(solution) => {
                let vars = vars.unwrap();
                for (var, value) in vars {
                    assert_eq!(value, solution.eval(var).unwrap());
                }
            }
            None => assert_eq!(None, vars),
        }
    }

    #[test]
    fn test_get_empty() {
        let objects = ObjectEncoding::empty();
        let mut model = aries_timelines::Sched::new(1, objects);
        let instance = instance_1();
        set_initial_state(&mut model, &instance);
        let param = ActParam {
            start: model.origin,
            end: 1.into(),
            presence: Lit::TRUE,
            source: None,
        };
        let var = get_empty_trailer_beluga(&mut model, &param, &instance);
        assert_var_value(&mut model, Some(vec![(var, 0)]));
        let jig_loc = JigLoc {
            held_by: Some((JigHolder::Hangar as i32).into()),
            num: Some(0.into()),
            pos: Some(0.into()),
        };
        effect_on_jig_loc(0.into(), &jig_loc, &mut model, &param);
        let var = get_empty_hangar(&mut model, &param, &instance);
        assert_var_value(&mut model, Some(vec![(var, 1)]));
        let jig_loc = JigLoc {
            held_by: Some((JigHolder::TrailerFactory as i32).into()),
            num: Some(0.into()),
            pos: Some(0.into()),
        };
        effect_on_jig_loc(0.into(), &jig_loc, &mut model, &param);
        let _var = get_empty_trailer_factory(&mut model, &param, &instance);
        assert_var_value(&mut model, None);
    }

    #[test]
    fn test_get_jig_from_jigtype() {
        let objects = ObjectEncoding::empty();
        let mut model = aries_timelines::Sched::new(1, objects);
        let instance = instance_1();
        set_initial_state(&mut model, &instance);
        let param = ActParam {
            start: model.origin,
            end: model.origin,
            presence: Lit::TRUE,
            source: None,
        };
        let var = get_jig_from_jigtype(1, &mut model, &param, &instance);
        assert_var_value(&mut model, Some(vec![(var, 1)]));
        let _var = get_jig_from_jigtype(2, &mut model, &param, &instance);
        assert_var_value(&mut model, None);
    }

    #[test]
    fn test_j_is_in() {
        let objects = ObjectEncoding::empty();
        let mut model = aries_timelines::Sched::new(1, objects);
        let instance = instance_1();
        set_initial_state(&mut model, &instance);
        let param = ActParam {
            start: model.origin,
            end: model.origin,
            presence: Lit::TRUE,
            source: None,
        };
        let mut vars: Vec<(Var, i32)> = vec![];
        for (b, flight) in instance.flights.iter().enumerate() {
            for &j in flight.incoming.iter() {
                let (holder, num) = j_is_in(j.into(), &mut model, &param, &instance);
                vars.push((holder, JigHolder::Incoming as i32));
                vars.push((num, b as i32));
            }
        }
        for (r, rack) in instance.racks.iter().enumerate() {
            for &j in rack.jigs.iter() {
                let (holder, num) = j_is_in(j.into(), &mut model, &param, &instance);
                vars.push((holder, JigHolder::Rack as i32));
                vars.push((num, r as i32));
            }
        }
        assert_var_value(&mut model, Some(vars));
    }

    //Test mod actions

    #[test]
    fn test_single_actions() {
        let objects = ObjectEncoding::empty();
        let mut model = aries_timelines::Sched::new(1, objects);
        let instance = instance_1();
        set_initial_state(&mut model, &instance);
        let task_id = model.add_task(Task {
            name: "actions".to_string(),
            start: 0.into(),
            end: 9.into(),
            presence: Lit::TRUE,
        });
        let param = ActParam {
            start: model.origin,
            end: 1.into(),
            presence: Lit::TRUE,
            source: Some(task_id),
        };

        new_unload_beluga_action(1.into(), 0.into(), 0.into(), &mut model, &param);
        let param = ActParam {
            start: 1.into(),
            end: 2.into(),
            presence: Lit::TRUE,
            source: Some(task_id),
        };
        let jig1_loc = JigLoc {
            held_by: Some((JigHolder::TrailerBeluga as i32).into()),
            num: Some(0.into()),
            pos: Some(0.into()),
        };
        cond_on_jig_loc(1.into(), &jig1_loc, &mut model, &param);

        new_put_down_rack(
            1.into(),
            0.into(),
            1.into(),
            Side::Beluga,
            &mut model,
            &param,
            &instance,
        );
        let param = ActParam {
            start: 2.into(),
            end: 3.into(),
            presence: Lit::TRUE,
            source: Some(task_id),
        };
        let jig1_loc = JigLoc {
            held_by: Some((JigHolder::Rack as i32).into()),
            num: Some(1.into()),
            pos: Some(0.into()),
        };
        cond_on_jig_loc(1.into(), &jig1_loc, &mut model, &param);

        new_pick_up_rack(
            1.into(),
            0.into(),
            1.into(),
            Side::Factory,
            &mut model,
            &param,
            &instance,
        );
        let param = ActParam {
            start: 3.into(),
            end: 4.into(),
            presence: Lit::TRUE,
            source: Some(task_id),
        };
        let jig1_loc = JigLoc {
            held_by: Some((JigHolder::TrailerFactory as i32).into()),
            num: Some(0.into()),
            pos: Some(0.into()),
        };
        cond_on_jig_loc(1.into(), &jig1_loc, &mut model, &param);

        new_deliver_to_hangar(1, 0.into(), 0.into(), 0.into(), &mut model, &param, &instance);
        let param = ActParam {
            start: 4.into(),
            end: 5.into(),
            presence: Lit::TRUE,
            source: Some(task_id),
        };
        let jig1_loc = JigLoc {
            held_by: Some((JigHolder::Hangar as i32).into()),
            num: Some(0.into()),
            pos: Some(0.into()),
        };
        cond_on_jig_loc(1.into(), &jig1_loc, &mut model, &param);

        new_get_from_hangar(1.into(), 0.into(), 0.into(), &mut model, &param);
        let param = ActParam {
            start: 5.into(),
            end: 6.into(),
            presence: Lit::TRUE,
            source: Some(task_id),
        };
        let jig1_loc = JigLoc {
            held_by: Some((JigHolder::TrailerFactory as i32).into()),
            num: Some(0.into()),
            pos: Some(0.into()),
        };
        cond_on_jig_loc(1.into(), &jig1_loc, &mut model, &param);

        new_put_down_rack(
            1.into(),
            0.into(),
            1.into(),
            Side::Factory,
            &mut model,
            &param,
            &instance,
        );
        let param = ActParam {
            start: 6.into(),
            end: 7.into(),
            presence: Lit::TRUE,
            source: Some(task_id),
        };
        let jig1_loc = JigLoc {
            held_by: Some((JigHolder::Rack as i32).into()),
            num: Some(1.into()),
            pos: Some(0.into()),
        };
        cond_on_jig_loc(1.into(), &jig1_loc, &mut model, &param);

        new_pick_up_rack(
            1.into(),
            0.into(),
            1.into(),
            Side::Beluga,
            &mut model,
            &param,
            &instance,
        );
        let param = ActParam {
            start: 7.into(),
            end: 8.into(),
            presence: Lit::TRUE,
            source: Some(task_id),
        };
        let jig1_loc = JigLoc {
            held_by: Some((JigHolder::TrailerBeluga as i32).into()),
            num: Some(0.into()),
            pos: Some(0.into()),
        };
        cond_on_jig_loc(1.into(), &jig1_loc, &mut model, &param);

        new_load_beluga_action(1.into(), 0.into(), 0.into(), &mut model, &param);
        let param = ActParam {
            start: 8.into(),
            end: 9.into(),
            presence: Lit::TRUE,
            source: Some(task_id),
        };
        let jig1_loc = JigLoc {
            held_by: Some((JigHolder::Outgoing as i32).into()),
            num: Some(0.into()),
            pos: Some(0.into()),
        };
        cond_on_jig_loc(1.into(), &jig1_loc, &mut model, &param);

        assert_var_value(&mut model, Some(vec![]));
    }
}
