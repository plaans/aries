use crate::{TspProblem, utils};

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

pub(crate) fn parse_tsp(input: &str, verbose: bool) -> TspProblem {
    let words = &mut utils::Parser::new(input);

    words.ignore_until_double_dot(String::from("NAME"));

    let name: String = words.pop();
    if verbose {
        println!("Parsing {}", name);
    }

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

    if verbose {
        println!("End parsing");
    }

    TspProblem { name, n, weights }
}
