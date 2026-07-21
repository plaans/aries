use super::*;
use std::collections::HashMap;
use std::error::Error;
use std::fs;

#[derive(Debug)]
pub struct JigType {
    pub name: String,
    pub size_empty: u32,
    pub size_loaded: u32,
}

#[derive(Debug)]
pub struct Jig {
    pub name: String,
    pub jig_type: JigTypeId,
    pub empty: bool,
}

#[derive(Debug)]
pub struct Trailer {
    pub name: String,
    pub side: Side,
}

#[derive(Debug)]
pub struct Rack {
    pub name: String,
    pub size: u32,
    pub jigs: Vec<JigId>,
}

#[derive(Debug)]
pub struct ProductionLine {
    pub name: String,
    pub schedule: Vec<JigId>,
}

#[derive(Debug)]
pub struct Flight {
    pub name: String,
    pub incoming: Vec<JigId>,
    pub outgoing: Vec<JigTypeId>,
}

#[derive(Debug)]
pub struct Instance {
    pub jig_types: Vec<JigType>,
    pub jigs: Vec<Jig>,
    pub trailers: Vec<Trailer>,
    pub hangars: Vec<String>,
    pub racks: Vec<Rack>,
    pub production_lines: Vec<ProductionLine>,
    pub flights: Vec<Flight>,
}

pub enum JigHolder { Incoming=0, Outgoing=1, Rack=2, Hangar=3, Trailer=4 }

impl Instance {
    pub fn build(filepath: &str) -> Result<Instance, Box<dyn Error>> {
        let contents = fs::read_to_string(filepath)?;
        let json_instance: json_instance::JsonInstance = serde_json::from_str(&contents)?;
        let mut instance = Instance {
            jig_types: vec![],
            jigs: vec![],
            trailers: vec![],
            hangars: vec![],
            racks: vec![],
            production_lines: vec![],
            flights: vec![],
        };
        //jig_types
        let (json, dict_jig_types) = map_to_vec(json_instance.jig_types);
        for j in json {
            instance.jig_types.push(JigType {
                name: j.name,
                size_empty: j.size_empty,
                size_loaded: j.size_loaded,
            });
        }
        //jigs
        let (json, dict_jigs) = map_to_vec(json_instance.jigs);
        for j in json {
            instance.jigs.push(Jig {
                name: j.name,
                jig_type: *dict_jig_types.get(&j.jig_type).unwrap(),
                empty: j.empty,
            })
        }
        //trailers
        for t in json_instance.trailers_beluga {
            instance.trailers.push(Trailer {
                name: t.name,
                side: Side::Beluga,
            })
        }
        for t in json_instance.trailers_factory {
            instance.trailers.push(Trailer {
                name: t.name,
                side: Side::Factory,
            })
        }
        //hangars
        for h in json_instance.hangars {
            instance.hangars.push(h);
        }
        //racks
        for r in json_instance.racks {
            let mut jigs: Vec<JigId> = vec![];
            for j in r.jigs {
                jigs.push(*dict_jigs.get(&j).unwrap());
            }
            instance.racks.push(Rack {
                name: r.name,
                size: r.size,
                jigs: jigs,
            });
        }
        //production lines
        for pl in json_instance.production_lines {
            let mut schedule: Vec<JigId> = vec![];
            for j in pl.schedule {
                schedule.push(*dict_jigs.get(&j).unwrap());
            }
            instance.production_lines.push(ProductionLine {
                name: pl.name,
                schedule,
            });
        }
        //flights
        for f in json_instance.flights {
            let mut incoming: Vec<JigId> = vec![];
            for j in f.incoming {
                incoming.push(*dict_jigs.get(&j).unwrap());
            }
            let mut outgoing: Vec<JigTypeId> = vec![];
            for jt in f.outgoing {
                outgoing.push(*dict_jig_types.get(&jt).unwrap());
            }
            instance.flights.push(Flight {
                name: f.name,
                incoming,
                outgoing,
            });
        }
        Ok(instance)
    }

    pub fn size_of_jig(&self, jig_id: JigId) -> Option<u32> {
        let jig_type;
        let jig_empty: bool;
        match self.jigs.get(jig_id) {
            None => return None,
            Some(jig) => {
                jig_type = jig.jig_type;
                jig_empty = jig.empty;
            }
        }
        let size: u32;
        match self.jig_types.get(jig_type) {
            None => return None,
            Some(jig_type) => match jig_empty {
                true => size = jig_type.size_empty,
                false => size = jig_type.size_loaded,
            },
        }
        Some(size)
    }

    //Get a vector of all the jigs we can send next to the production lines
    //pl_state : key=production_line (index in production_lines) ; v = jig (index in pl.schedule)
    pub fn next_in_production(&self, pl_state: HashMap<ProdLineId, usize>) -> Vec<JigId> {
        let mut next: Vec<JigId> = vec![];
        for (pl_i, jig_i) in pl_state {
            next.push(self.production_lines[pl_i].schedule[jig_i]);
        }
        next
    }

    pub fn next_in_beluga_outgoing(&self, current_beluga : BelugaId, content : &Vec<JigId>) -> JigTypeId {
        *&self.flights[current_beluga].outgoing[content.len()]
    }

    pub fn fits_on_rack(&self, rack : RackId, rack_content : Vec<JigId>, new_jig : JigId) -> bool {
        let mut sum : u32 = 0;
        for j in rack_content {
            sum += &self.size_of_jig(j).unwrap();
        }
        sum += &self.size_of_jig(new_jig).unwrap();
        sum <= *&self.racks[rack].size
    }

    pub fn bounds_jig_holder(&self) -> (i32, i32) {
        let (lb, mut ub) : (i32, i32) = (0,0);
        ub = ub.max(self.bounds_incoming().0);
        ub = ub.max(self.bounds_outgoing().0);
        ub = ub.max(self.bounds_rack().0);
        ub = ub.max(self.bounds_hangar().0);
        ub = ub.max(self.bounds_trailer().0);
        (lb, ub)
    }

    pub fn bounds_incoming(&self) -> (i32, i32) {
        let mut ub : i32 = 0;
        for f in self.flights.iter() {
            ub = ub.max(f.incoming.len() as i32);
        }
        (0, ub)
    }
    pub fn bounds_outgoing(&self) -> (i32, i32) {
        let mut ub : i32 = 0;
        for f in self.flights.iter() {
            ub = ub.max(f.outgoing.len() as i32);
        }
        (0, ub)
    }
    pub fn bounds_trailer(&self) -> (i32, i32) {
        (0, (&self.trailers.len()-1) as i32)
    }
    pub fn bounds_rack(&self) -> (i32, i32) {
        (0, (&self.racks.len()-1) as i32)
    }
    pub fn bounds_hangar(&self) -> (i32, i32) {
        (0, (&self.hangars.len()-1) as i32)
    }
    pub fn bounds_jig(&self) -> (i32, i32) {
        (0, (&self.jigs.len()-1) as i32)
    }
}

// Transforms a HashMap<String, T> into a Vec<T>, with a HashMap linking the initial String key with the index of the initial value
fn map_to_vec<T>(hash: HashMap<String, T>) -> (Vec<T>, HashMap<String, usize>) {
    let mut pairs: Vec<(String, T)> = hash.into_iter().collect();
    pairs.sort_by(|p1, p2| p1.0.cmp(&p2.0)); //alphabetical order
    let dict: HashMap<String, usize> = pairs
        .iter()
        .enumerate()
        .map(|(i, (name, _x))| (String::from(name), i))
        .collect();
    let vector: Vec<T> = pairs.into_iter().map(|(_k, v)| v).collect();
    (vector, dict)
}

#[cfg(test)]
mod test {
    use super::*;

    fn instance_for_tests() -> Instance {
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
            ],
            trailers: vec![],
            hangars: vec![],
            racks: vec![],
            production_lines: vec![ProductionLine {
                name: String::from("pl0"),
                schedule: vec![0, 1],
            }],
            flights: vec![],
        }
    }

    #[test]
    fn test_size_of_jig() {
        let instance = instance_for_tests();
        assert_eq!(4, instance.size_of_jig(0).unwrap());
        assert_eq!(11, instance.size_of_jig(1).unwrap());
    }

    #[test]
    fn test_next_jigs_in_pl() {
        let instance = instance_for_tests();
        let state = HashMap::from([(0, 1)]);
        let expected = vec![1];
        assert_eq!(expected, instance.next_in_production(state));
    }
}
