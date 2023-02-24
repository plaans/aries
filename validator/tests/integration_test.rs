mod common;

#[cfg(test)]
mod test {
    use super::common;
    use aries_plan_validator_derive::generate_tests;

    generate_tests!(["planning/ext/up/bins/problems/", "timed_connected_locations"]);
}
