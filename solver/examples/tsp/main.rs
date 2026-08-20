use std::{fs, path::Path};

use aries_solver::prelude::*;

#[path = "../utils/mod.rs"]
mod utils;

pub struct DirectedSegmentMap<T> {
    n: usize,
    data: Vec<T>,
}

impl<T> DirectedSegmentMap<T> {
    pub fn new(n: usize, default_val: T) -> Self
    where
        T: Clone,
    {
        assert!(n > 1, "At least 2 nodes are necessary");
        let len = n * (n - 1);
        Self {
            n,
            data: vec![default_val; len],
        }
    }

    pub fn new_with<F>(n: usize, mut init: F) -> Self
    where
        F: FnMut() -> T,
    {
        assert!(n > 1, "At least 2 nodes are necessary");
        let len = n * (n - 1);
        let mut data = Vec::with_capacity(len);

        for i in 0..n {
            for j in 0..n {
                if i != j {
                    data.push(init());
                }
            }
        }

        Self { n, data }
    }

    #[inline]
    fn index(&self, i: usize, j: usize) -> usize {
        assert!(i < self.n && j < self.n, "Index out of bounds");
        assert!(i != j, "Line and column index must be different");

        // If j is greater than i, we shift it by -1 to fill the gap for i==j
        let j_adj = if j > i { j - 1 } else { j };
        i * (self.n - 1) + j_adj
    }

    pub fn get(&self, i: usize, j: usize) -> &T {
        &self.data[self.index(i, j)]
    }

    pub fn get_mut(&mut self, i: usize, j: usize) -> &mut T {
        let idx = self.index(i, j);
        &mut self.data[idx]
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }
}

#[derive(Copy, Clone, Debug)]
struct EucPoint {
    x: f64,
    y: f64,
}

impl EucPoint {
    /// Euclidean distance between two points
    fn dist(&self, other: EucPoint) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

#[derive(Copy, Clone, Debug)]
struct GeoPoint {
    // Both are in rad
    latitude: f64,
    longitude: f64,
}

impl GeoPoint {
    fn new_from_deg(lat_deg: f64, long_deg: f64) -> Self {
        let latitude: f64;
        let longitude: f64;

        {
            let deg = lat_deg.trunc();
            let min = lat_deg - deg;

            latitude = std::f64::consts::PI * (deg + 5.0 * min / 3.0) / 180.0;
        }

        {
            let deg = long_deg.trunc();
            let min = long_deg - deg;

            longitude = std::f64::consts::PI * (deg + 5.0 * min / 3.0) / 180.0;
        }

        GeoPoint { latitude, longitude }
    }

    /// Compute the distance between 2 GeoPoint
    fn dist(&self, other: GeoPoint) -> f64 {
        const EARTH_RADIUS: f64 = 6378.388;

        let q1 = f64::cos(self.longitude - other.longitude);
        let q2 = f64::cos(self.latitude - other.latitude);
        let q3 = f64::cos(self.latitude + other.latitude);

        (EARTH_RADIUS * f64::acos(0.5 * ((1.0 + q1) * q2 - (1.0 - q1) * q3)) + 1.0).floor()
    }
}

#[derive(Debug)]
struct TspProblem {
    /// Number of nodes
    n: usize,
    /// Set of weight between nodes
    weights: Vec<Vec<f64>>,
}

#[derive(Debug)]
struct TspSolution {
    /// Length of the tour
    cost: f64,
    /// Corresponds to the order in wich the cities has to be travelled
    tour_order: Vec<usize>,
}

/// Constant by which to multiply distances to get an integer without losing to much precision
const SCALE_FACTOR: f64 = 1000.0;

/// Solves a tsp problem and returns a TspSolution.
///
/// It bases one the One Commodity Flow explain in the following paper: https://matmod.ch/lpl/PDF/tsp-2.pdf
fn solve_tsp(pb: &TspProblem, deadline: impl Into<f64>) -> Option<TspSolution> {
    let mut model = Model::new();

    let trav_edg = DirectedSegmentMap::new_with(pb.n, || model.new_variable(0, 1));

    let flow_edg = DirectedSegmentMap::new_with(pb.n, || model.new_variable(0, pb.n as IntCst - 1));

    let mut total_cost = LinSum::zero();

    for i in 0..pb.n {
        let mut sum_lin_trav_edg = LinSum::zero();
        let mut sum_col_trav_edg = LinSum::zero();

        let mut sum_lin_flow_edg = LinSum::zero();
        let mut sum_col_flow_edg = LinSum::zero();

        for j in 0..pb.n {
            if i == j {
                continue;
            }

            total_cost += *trav_edg.get(i, j) * (pb.weights[i][j] * SCALE_FACTOR).ceil() as IntCst;
            sum_lin_trav_edg += *trav_edg.get(i, j);
            sum_col_trav_edg += *trav_edg.get(j, i);

            model.enforce(leq(*flow_edg.get(i, j), (pb.n as IntCst - 1) * *trav_edg.get(i, j))); // D3

            if i != 0 && j != 0 {
                model.enforce(leq(*flow_edg.get(i, j), (pb.n as IntCst - 2) * *trav_edg.get(i, j))); // G
            }

            sum_lin_flow_edg += *flow_edg.get(i, j);

            if j != 0 {
                sum_col_flow_edg += *flow_edg.get(j, i);
            }

            if j > i {
                model.enforce(leq(LinSum::zero() + *trav_edg.get(i, j) + *trav_edg.get(j, i), 1)); // S
            }
        }

        // We force our nodes to be visited exactly once
        model.enforce(eq(sum_lin_trav_edg, 1)); // A
        model.enforce(eq(sum_col_trav_edg, 1)); // B

        if i == 0 {
            model.enforce(eq(sum_lin_flow_edg, pb.n as IntCst - 1)); // D2
        } else {
            model.enforce(eq(sum_lin_flow_edg - sum_col_flow_edg, 1)); // D1
        }
    }

    let total_cost_var = model.new_variable(0, INT_CST_MAX);

    model.enforce(eq(total_cost, total_cost_var));

    println!("Solving...");

    // create the solver and solve to optimal (with 180s timeout)
    let mut solver = Solver::new(model);
    match solver.minimize_with_callback(
        total_cost_var,
        |_, sol| {
            println!(
                "New solution with cost: {}",
                sol.eval(total_cost_var).unwrap() as f64 / SCALE_FACTOR
            )
        },
        SearchLimit::duration_secs(deadline),
    ) {
        Ok(Some((_, sol))) => {
            println!("== Optimal solution found ==");

            let mut cost = 0.0;

            let mut tour_order = Vec::<usize>::new();

            let mut curr_idx = 0;

            'while_loop: while tour_order.len() != pb.n {
                tour_order.push(curr_idx + 1); // We shift the index as TSP problems start numerotation with 1

                for next_idx in 0..pb.n {
                    if curr_idx == next_idx {
                        continue;
                    }

                    if sol.eval(*trav_edg.get(curr_idx, next_idx)).unwrap() == 1 {
                        cost += pb.weights[curr_idx][next_idx];
                        curr_idx = next_idx;
                        continue 'while_loop;
                    }
                }

                panic!("No following node found, the solution contains an error");
            }

            solver.print_stats();

            Some(TspSolution { cost, tour_order })
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

fn parse(input: &str) -> TspProblem {
    let words = &mut utils::Parser::new(input);

    words.ignore_until_double_dot(String::from("NAME"));

    let name_instance: String = words.pop();
    println!("Parsing {}", name_instance);

    words.ignore_until_double_dot(String::from("TYPE"));
    words.ignore_expected(String::from("TSP"));

    words.ignore_until_double_dot(String::from("DIMENSION"));
    let n = words.pop();

    words.ignore_until_double_dot(String::from("EDGE_WEIGHT_TYPE"));

    let weight_type: String = words.pop();

    let mut weights = vec![vec![0.0; n]; n];

    match weight_type.as_str() {
        "EUC_2D" => {
            words.ignore_until(String::from("NODE_COORD_SECTION"));

            let mut points = Vec::new();

            for i in 1..=n {
                words.ignore_expected(i);

                let point = EucPoint {
                    x: words.pop(),
                    y: words.pop(),
                };

                points.push(point);
            }

            for i in 0..n {
                for j in i + 1..n {
                    let weight = points[i].dist(points[j]);
                    weights[i][j] = weight;
                    weights[j][i] = weight;
                }
            }
        }

        "GEO" => {
            words.ignore_until(String::from("NODE_COORD_SECTION"));

            let mut points = Vec::new();

            for i in 1..=n {
                words.ignore_expected(i);

                let lat = words.pop();
                let long = words.pop();

                let point = GeoPoint::new_from_deg(lat, long);

                points.push(point);
            }

            for i in 0..n {
                for j in i + 1..n {
                    let weight = points[i].dist(points[j]);
                    weights[i][j] = weight;
                    weights[j][i] = weight;
                }
            }
        }

        "EXPLICIT" => {
            words.ignore_until_double_dot(String::from("EDGE_WEIGHT_FORMAT"));
            let weight_format: String = words.pop();

            words.ignore_until(String::from("EDGE_WEIGHT_SECTION"));

            match weight_format.as_str() {
                "FULL_MATRIX" => {
                    for row in weights.iter_mut().take(n) {
                        for cell in row.iter_mut().take(n) {
                            *cell = words.pop();
                        }
                    }
                }

                "UPPER_ROW" =>
                {
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..n {
                        for j in i + 1..n {
                            let weight = words.pop();
                            weights[i][j] = weight;
                            weights[j][i] = weight;
                        }
                    }
                }

                "LOWER_ROW" =>
                {
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..n {
                        for j in 0..i {
                            let weight = words.pop();
                            weights[i][j] = weight;
                            weights[j][i] = weight;
                        }
                    }
                }

                "UPPER_DIAG_ROW" =>
                {
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..n {
                        for j in i..n {
                            let weight = words.pop();
                            weights[i][j] = weight;
                            weights[j][i] = weight;
                        }
                    }
                }

                "LOWER_DIAG_ROW" =>
                {
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..n {
                        for j in 0..=i {
                            let weight = words.pop();
                            weights[i][j] = weight;
                            weights[j][i] = weight;
                        }
                    }
                }

                _ => panic!("Unvalid weight_format {weight_format}"),
            }
        }
        _ => panic!("Unvalid weight_type {weight_type}"),
    }

    // println!("Weights:\n {:?}", weights);

    println!("End parsing");

    TspProblem { n, weights }
}

fn solve_tsp_from_file<P>(path: P, deadline: impl Into<f64>) -> Option<TspSolution>
where
    P: AsRef<Path>,
{
    let problem_str = fs::read_to_string(path).expect("No such file");

    let pb = parse(&problem_str);

    // println!("Problem: {:?}", pb);

    let solution = solve_tsp(&pb, deadline);

    println!("Solution: {:?}", solution);

    solution
}

const PATH_INSTANCES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/tsp/instances");

fn main() {
    for entry in fs::read_dir(PATH_INSTANCES).expect("Error while reading directory") {
        let entry = entry.expect("Error while reading file");
        let path = entry.path();

        if path.is_file() {
            solve_tsp_from_file(path, 30);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_burma14() {
        let opt_solution = solve_tsp_from_file(PATH_INSTANCES.to_owned() + "/burma14.tsp", 300);

        if let Some(solution) = opt_solution {
            assert_eq!(solution.cost, 3323.0, "Optimal cost differs from the expected");

            let expected_tour_order1 = vec![1, 2, 14, 3, 4, 5, 6, 12, 7, 13, 8, 11, 9, 10];
            let expected_tour_order2 = vec![1, 10, 9, 11, 8, 13, 7, 12, 6, 5, 4, 3, 14, 2];

            assert!(
                solution.tour_order == expected_tour_order1 || solution.tour_order == expected_tour_order2,
                "Unvalid tour order"
            )
        } else {
            println!("Timeout reached, solution can't be tested");
        }
    }

    #[test]
    fn test_ulysses16() {
        let opt_solution = solve_tsp_from_file(PATH_INSTANCES.to_owned() + "/ulysses16.tsp", 300);

        if let Some(solution) = opt_solution {
            assert_eq!(solution.cost, 6859.0, "Optimal cost differs from the expected");

            let expected_tour_order1 = vec![1, 14, 13, 12, 7, 6, 15, 5, 11, 9, 10, 16, 3, 2, 4, 8];
            let expected_tour_order2 = vec![1, 8, 4, 2, 3, 16, 10, 9, 11, 5, 15, 6, 7, 12, 13, 14];

            assert!(
                solution.tour_order == expected_tour_order1 || solution.tour_order == expected_tour_order2,
                "Unvalid tour order"
            )
        } else {
            println!("Timeout reached, solution can't be tested");
        }
    }

    #[ignore = "Too long to run by default"]
    #[test]
    fn test_gr17() {
        println!("{}", PATH_INSTANCES.to_owned() + "/gr17.tsp");

        let opt_solution = solve_tsp_from_file(PATH_INSTANCES.to_owned() + "/gr17.tsp", 300);

        if let Some(solution) = opt_solution {
            assert_eq!(solution.cost, 2085.0, "Optimal cost differs from the expected");
        } else {
            println!("Timeout reached, solution can't be tested");
        }
    }
}
