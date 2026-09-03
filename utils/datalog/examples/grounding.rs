//! This example simulates the grounding of a PDDL action.
use aries_datalog::*;

fn main() {
    let mut prog = Program::new();

    let loc = prog.new_predicate(1);
    loc.add([1]); // loc(l1).   (1 is the id of a symbol l1)
    loc.add([2]); // loc(l2).
    loc.add([3]);
    loc.add([4]);
    loc.add([6]);
    loc.add([7]);

    let robot = prog.new_predicate(1);
    robot.add([11]); // robot(r1).   (11 is the ID of the symbol r1)
    robot.add([12]); // robot(r2).
    robot.add([13]);
    robot.add([14]);
    robot.add([16]);

    let connected = prog.new_predicate(2);
    connected.add([1, 2]); // connected(l1, l2).
    connected.add([2, 3]);
    connected.add([3, 4]);
    connected.add([1, 2]);
    connected.add([2, 1]);
    connected.add([3, 2]);
    connected.add([4, 3]);
    connected.add([2, 1]);
    connected.add([6, 7]); // disconnected component l6, l7
    connected.add([7, 6]);

    let at = prog.new_predicate(2);
    at.add([11, 2]); // at(r1, l2).
    at.add([12, 4]); // at(r2, l4).
    at.add([16, 7]);

    use Arg::*;

    let move_applicable = prog.new_predicate(3);

    // move_applicable(?r, ?l1, l2) :-
    //   robot(?r),
    //   loc(?l1),
    //   loc(?l2),
    //   at(?r, ?l1)
    //   connected(?l1, ?l2).
    let move_rule = Rule::new(
        move_applicable.apply([Var(0), Var(1), Var(2)]),
        [
            robot.apply([Var(0)]),
            loc.apply([Var(1)]),
            loc.apply([Var(2)]),
            at.apply([Var(0), Var(1)]),
            connected.apply([Var(1), Var(2)]),
        ],
    );
    prog.add_rule(move_rule);

    // at(?r, ?l) :- move_applicable(?r, _, ?l)
    prog.add_rule(Rule::new(
        at.apply([Var(0), Var(2)]),
        [move_applicable.apply([Var(0), Var(1), Var(2)])],
    ));

    // goal predicates (mainly here to demonstrate 0-arity predicates)
    let g1 = prog.new_predicate(0);
    prog.add_rule(Rule::new(
        g1.apply([]),
        [at.apply([Arg::Sym(11), Arg::Sym(1)])], // goal1: robot r1 can reach location l1
    ));
    let g2 = prog.new_predicate(0);
    prog.add_rule(Rule::new(
        g2.apply([]),
        [at.apply([Arg::Sym(11), Arg::Sym(4)])], // goal1: robot r1 can reach location l4
    ));
    let g3 = prog.new_predicate(0);
    prog.add_rule(Rule::new(
        g3.apply([]),
        [at.apply([Arg::Sym(11), Arg::Sym(7)])], // goal1: robot r1 can reach location 7
    ));
    // g1g2 :- g1, g2.
    let g1g2 = prog.new_predicate(0);
    prog.add_rule(Rule::new(g1g2.apply([]), [g1.apply([]), g2.apply([])]));
    // all_goals : g1g2, g3.
    let all_goals = prog.new_predicate(0);
    prog.add_rule(Rule::new(all_goals.apply([]), [g1g2.apply([]), g3.apply([])]));

    // run inference until completion
    prog.run();

    // access the resulting variables

    println!("\n == reachable locations ==\n");
    at.extract().rows().for_each(|row| println!("at{row:?}"));

    println!("\n == applicable actions ==\n");
    move_applicable.extract().rows().for_each(|row| println!("move{row:?}"));

    println!("\n == goals ==\n");
    g1.extract().rows().for_each(|row| println!("g1{row:?}"));
    g2.extract().rows().for_each(|row| println!("g2{row:?}"));
    g3.extract().rows().for_each(|row| println!("g3{row:?}"));
    g1g2.extract().rows().for_each(|row| println!("g1&g2{row:?}"));
    all_goals.extract().rows().for_each(|row| println!("all_goals{row:?}"));

    assert!(!g1g2.extract().is_empty()); // should hold <=> table contains the unit row
    assert!(all_goals.extract().is_empty()); // should not hold <=> table is empty
}
