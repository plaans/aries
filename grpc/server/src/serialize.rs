// This module parses the GRPC service definition into a set of Rust structs.
use aries_core::state::Domains;
use std::sync::Arc;

pub fn serialize_answer(_assignments: Option<Arc<Domains>>) -> aries_grpc_api::Answer {
    unimplemented!("Serializing the plan into UP Answer is not yet completed")
}
