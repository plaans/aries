mod utils;

use itertools::Itertools;

use aries_solver::lang::ModelView;
use aries_solver::prelude::*;

/// Representation of an Orienteering problem.
///
/// The objective is to find a path (with fixed departure and arrival) going through
/// a subset of locations such that:
///
///  - the duration of the path (~traveled distance) does not exceed a maximum value
///  - the reward collected from all visited locations is maximal.
struct OrienteeringProblem {
    /// Max duration (= distance) of the path
    tmax: f64,
    /// Departure of the agent
    departure: Loc,
    /// Arrival of the agent
    arrival: Loc,
    /// Set of point interest, each with a location and a reward that is gained when visiting it.
    points_of_interest: Vec<Poi>,
}

/// Point of interest: location with associated reward.
struct Poi {
    loc: Loc,
    reward: IntCst,
}

#[derive(Copy, Clone)]
struct Loc {
    x: f64,
    y: f64,
}
impl Loc {
    /// Euclidean distance between two locations
    fn dist(&self, other: Loc) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// Constant by which to multiply distances/times to get an integer without losing to much precision
const SCALE_FACTOR: f64 = 1000.0;

/// Solves an orienteering problem and returns the optimal reward.
fn solve_orienteering(pb: &OrienteeringProblem) -> Option<IntCst> {
    let mut model = Model::new();

    // converts the maximum time to a scaled up integer (allowing up to three decimals).
    // We take the first integer below to make sur we do not overestimate the deadline
    let tmax = (pb.tmax * SCALE_FACTOR).floor() as IntCst;

    // struct that will gather all necessary information to encode represent point on the path (include departure and arrival)
    struct Visit {
        visited: Lit, // presence variable: true iff the point is part of the solution
        time: Var,    // visit time optional variable: if visited, will indicate the time at which the visit is done
        loc: Loc,
        reward: IntCst,
    }
    let mut visits = vec![];
    // visit of departure point
    visits.push(Visit {
        visited: Lit::TRUE, // always visited
        time: Var::ZERO,    // visited at t=0
        loc: pb.departure,
        reward: 0,
    });
    // visit of the arrival point
    visits.push(Visit {
        visited: Lit::TRUE, // always visited
        time: model.new_variable(0, tmax),
        loc: pb.arrival,
        reward: 0,
    });
    for visit in &pb.points_of_interest {
        // create a new literal that will encode whether the point is visited
        let visited = model.new_optional_bool_var(Lit::TRUE);

        // create a new *optional* variable that represent the time at which the point is visited
        // The variable is *optional*, with `presence` as its scope.
        let time = model.new_optional_variable(0, tmax, visited);

        // note: one can check that the presence literal of `time` is indeed `visited`
        debug_assert_eq!(model.presence(time), visited);

        // enforce that the the visit is done before the arriving at the destination
        model.enforce_scoped(lt(time, visits[1].time), [visited]);

        visits.push(Visit {
            visited,
            time,
            loc: visit.loc,
            reward: visit.reward,
        });
    }

    for (i, vi) in visits.iter().enumerate() {
        for vj in &visits[i + 1..] {
            // we need to enforce a minimum delay between the two visits

            // convert the distance (f64) into a scaled up integer.
            // We take the ceil to make sure we do not underestimate the travel time
            let dist = (vi.loc.dist(vj.loc) * SCALE_FACTOR).ceil() as IntCst;

            // get a new presence variable p_ij <=> (p_i && p_j)
            // This variable will be true when both visits are made
            let present_ij = model.get_conjunctive_scope(&[vi.visited, vj.visited]);

            // create a decision variable with scope `p_ij`.
            // When `p_ij` is true, a boolean value will need to be assigned to this one.
            // The value will decide which of `vi` and `vj` precedes the other.
            let before_ij = model.new_optional_bool_var(present_ij);

            // post `b_ij => (vi precedes vj)` ... accounting for travel time
            model.enforce_if(before_ij, leq(vi.time + dist, vj.time));
            // post `not b_ij => (vj precedes vi)` ... accounting for travel time
            model.enforce_if(!before_ij, leq(vj.time + dist, vi.time));
        }
    }

    // create a linear expression encoding the reward
    let mut total_reward = LinSum::zero();
    for v in &visits {
        // the reward is collected if the visited variable is true
        total_reward += v.reward * bool2int(v.visited, &mut model);
    }
    // The solver requires a variable as the optimization objective.
    // Create a new variable that is always equal the the objective sum.
    let total_reward_var = model.new_variable(0, INT_CST_MAX);
    model.enforce(eq(total_reward_var, total_reward));

    // create the solver and solve to optimal (with 180s timeout)
    let mut solver = Solver::new(model);
    match solver.maximize_with_callback(
        total_reward_var,
        |_, sol| println!("New solution with reward: {}", sol.eval(total_reward_var).unwrap()),
        SearchLimit::duration_secs(180),
    ) {
        Ok(Some((_, sol))) => {
            println!("== Optimal solution found ==");

            // gather all visited points
            let mut visited = vec![];
            for (i, v) in visits.iter().enumerate() {
                // eval will return an option contains:
                //  - Some(v) if the variable is present and has the value v
                //  - None if the variable is absent
                if let Some(visit_time) = sol.eval(v.time) {
                    // variable is present, indicating that the location is visited at time `visit_time`
                    visited.push((visit_time, i));

                    // note: we could also have checked that the location is visited by the checking presence variable.
                    debug_assert_eq!(sol.eval(v.visited), Some(true));
                }
            }
            visited.sort(); // sort by visit time
            println!("Path: {}", visited.iter().map(|(_t, i)| i).format(" "));

            let collected_reward = sol.eval(total_reward_var).unwrap();
            println!("Collected reward: {collected_reward}");
            println!("Total time: {:.3}", visited.last().unwrap().0 as f64 / SCALE_FACTOR);
            Some(collected_reward)
        }
        Ok(None) => {
            println!("No solution");
            None
        }
        Err(_) => {
            println!("timeout");
            None
        }
    }
}

fn parse(input: &str) -> OrienteeringProblem {
    let words = &mut utils::Parser::new(input);

    // parse `tmax n_path`, the maximum allowed time and number of path (always 1 for orienteering)
    let tmax = words.pop();
    words.ignore_expected(1);

    // parse start location
    let start = Loc {
        x: words.pop(),
        y: words.pop(),
    };
    words.ignore_expected(0);

    // parse final location
    let end = Loc {
        x: words.pop(),
        y: words.pop(),
    };
    words.ignore_expected(0);

    // parse all lines `x y reward` for each point of interest
    let mut pois = vec![];
    while !words.is_empty() {
        pois.push(Poi {
            loc: Loc {
                x: words.pop(),
                y: words.pop(),
            },
            reward: words.pop(),
        })
    }

    OrienteeringProblem {
        tmax,
        departure: start,
        arrival: end,
        points_of_interest: pois,
    }
}

fn main() {
    let pb = parse(SIMPLE_PROBLEM);
    solve_orienteering(&pb);
}

/// A simple orienteering problem
///
/// [Source and format](https://www.mech.kuleuven.be/en/mim/op#autotoc-item-autotoc-2)
const SIMPLE_PROBLEM: &str = "25	1
10.5	14.4	0
11.2	14.1	0
18	15.9	10
18.3	13.3	10
16.5	9.3	10
15.4	11	10
14.9	13.2	5
16.3	13.3	5
16.4	17.8	5
15	17.9	5
16.1	19.6	10
15.7	20.6	10
13.2	20.1	10
14.3	15.3	5
14	5.1	10
11.4	6.7	15
8.3	5	15
7.9	9.8	10
11.4	12	5
11.2	17.6	5
10.1	18.7	5
11.7	20.3	10
10.2	22.1	10
9.7	23.8	10
10.1	26.4	15
7.4	24	15
8.2	19.9	15
8.7	17.7	10
8.9	13.6	10
5.6	11.1	10
4.9	18.9	10
7.3	18.8	10";

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_output() {
        let pb = parse(SIMPLE_PROBLEM);
        assert_eq!(solve_orienteering(&pb), Some(90))
    }
}
