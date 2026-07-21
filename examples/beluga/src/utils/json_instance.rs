use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct Trailer {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct JigType {
    pub name: String,
    pub size_empty: u32,
    pub size_loaded: u32,
}

#[derive(Deserialize, Debug)]
pub struct Rack {
    pub name: String,
    pub size: u32,
    pub jigs: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct Jig {
    pub name: String,
    #[serde(rename = "type")] // type is a Rust keyword
    pub jig_type: String,
    pub empty: bool,
}

#[derive(Deserialize, Debug)]
pub struct ProductionLine {
    pub name: String,
    pub schedule: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct Flight {
    pub name: String,
    pub incoming: Vec<String>,
    pub outgoing: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct JsonInstance {
    pub trailers_beluga: Vec<Trailer>,
    pub trailers_factory: Vec<Trailer>,
    pub hangars: Vec<String>,
    pub jig_types: HashMap<String, JigType>,
    pub racks: Vec<Rack>,
    pub jigs: HashMap<String, Jig>,
    pub production_lines: Vec<ProductionLine>,
    pub flights: Vec<Flight>,
}
