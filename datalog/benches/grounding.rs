use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use aries_datalog::*;

fn ground(num_locs: u32, num_bots: u32) -> usize {
    let mut prog = Program::new();

    let robot = prog.new_predicate(1);
    let loc = prog.new_predicate(1);
    let connected = prog.new_predicate(2);
    let at = prog.new_predicate(2);

    for l in 1..=num_locs {
        loc.add([l]);
        connected.add([l, l + 1]); // connected(l1, l2).
    }

    for r in 1..=num_bots {
        robot.add([r]);
        at.add([r, 1]);
    }



    use Arg::*;

    let move_applicable = prog.new_predicate(3);

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

    // run inference until completion
    prog.run();

    move_applicable.extract().rows().count()
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("ground 10/2", |b| b.iter(|| ground(black_box(10), black_box(2))));
    c.bench_function("ground 30/5", |b| b.iter(|| ground(black_box(30), black_box(5))));
    // c.bench_function("ground 100/20", |b| b.iter(|| ground(black_box(100), black_box(20))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
