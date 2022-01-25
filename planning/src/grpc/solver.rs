use crate::grpc::serialize::{Answer_, Problem_};
use anyhow::Error;

pub fn solve(problem: Problem_) -> Result<Answer_, Error> {
    let answer = Answer_::default();

    // Assert that the problem is of desired type.

    Ok(answer)
}
