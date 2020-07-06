#![allow(dead_code)]

pub mod cesta;

use aries_collections::id_map::IdMap;
use std::marker::PhantomData;

use std::fmt::Display;
use std::ops::{Add, AddAssign, Neg, Sub};

/// TODO: this type is an aberration
pub trait FloatLike:
    Add<Self, Output = Self> + Display + Copy + Ord + Sub<Self, Output = Self> + Neg<Output = Self> + AddAssign<Self>
{
    fn zero() -> Self;
    fn infty() -> Self;
    fn neg_infty() -> Self;
    fn epsilon() -> Self;
}

impl FloatLike for i32 {
    fn zero() -> Self {
        0
    }

    fn infty() -> Self {
        std::i32::MAX / 2
    }

    fn neg_infty() -> Self {
        std::i32::MIN / 2
    }

    fn epsilon() -> Self {
        1
    }
}

#[derive(Clone, Debug)]
struct Dist<W> {
    pub forward: W,
    forward_cause: Option<CId>,
    pub backward: W,
    backward_cause: Option<CId>,
}
impl<W: FloatLike> Dist<W> {
    fn default() -> Self {
        Dist {
            forward: W::infty(),
            forward_cause: None,
            backward: W::neg_infty(),
            backward_cause: None,
        }
    }

    fn zero() -> Self {
        Dist {
            forward: W::zero(),
            forward_cause: None,
            backward: W::zero(),
            backward_cause: None,
        }
    }
}

#[derive(Debug)]
struct Distances<N, W> {
    pub dists: Vec<Dist<W>>,
    node: PhantomData<N>,
}
impl<N: Into<usize>, W: FloatLike> Distances<N, W> {
    pub fn new(source: N, n: usize) -> Self {
        let mut dists = vec![Dist::default(); n];
        dists[source.into()] = Dist::zero();
        Distances {
            dists,
            node: PhantomData,
        }
    }

    pub fn to(&self, n: N) -> W {
        self.dists[n.into()].forward
    }
    pub fn from(&self, n: N) -> W {
        self.dists[n.into()].backward
    }
}

/// X <= Y + w
#[derive(Clone, Copy, Debug)]
pub struct LEQ<N, W> {
    x: N,
    y: N,
    w: W,
}

type VId = usize;
const ORIGIN: VId = 0;

pub struct STN<N, W> {
    variables: Vec<(Dom<W>, Option<N>)>,
    external_vars: IdMap<N, VId>,
    constraints: Vec<Const<W>>,
}

#[derive(Copy, Clone, Debug)]
pub struct Dom<W> {
    pub min: W,
    pub max: W,
}
#[derive(Copy, Clone, Debug)]
struct Const<W> {
    internal: bool,
    active: bool,
    c: LEQ<VId, W>,
}

#[allow(clippy::new_without_default)]
impl<N: Into<usize> + Copy, W: FloatLike> STN<N, W> {
    pub fn new() -> Self {
        let mut variables = Vec::with_capacity(16);
        let d_zero = Dom {
            min: W::zero(),
            max: W::zero(),
        };
        variables.push((d_zero, None)); // reserve first slot for origin
        STN {
            variables,
            external_vars: IdMap::new(),
            constraints: Vec::with_capacity(16),
        }
    }

    #[inline]
    fn origin(&self) -> VId {
        ORIGIN
    }

    pub fn add_node(&mut self, label: N, min: W, max: W) {
        assert!(!self.external_vars.contains_key(label));
        assert!(W::neg_infty() <= min && max <= W::infty());
        let id = self.variables.len();
        self.variables.push((Dom { min, max }, Some(label)));
        self.external_vars.insert(label, id);
        self.constraints.push(Const {
            internal: true,
            active: true,
            c: LEQ {
                x: self.origin(),
                y: id,
                w: -min,
            },
        });
        self.constraints.push(Const {
            internal: true,
            active: true,
            c: LEQ {
                x: id,
                y: self.origin(),
                w: max,
            },
        });
    }

    pub fn record_constraint(&mut self, x: N, y: N, w: W, active: bool) -> CId {
        let xi = self.external_vars[x];
        let yi = self.external_vars[y];
        self.constraints.push(Const {
            internal: false,
            active,
            c: LEQ { x: xi, y: yi, w },
        });
        self.constraints.len() - 1
    }

    pub fn set_active(&mut self, cid: CId, active: bool) {
        self.constraints[cid].active = active;
    }

    pub fn constraints(&self) -> impl Iterator<Item = CId> {
        0..self.constraints.len()
    }
}

/// Identifier of a constraint
type CId = usize;

pub fn domains<N, W>(stn: &STN<N, W>) -> Result<IdMap<N, Dom<W>>, Vec<CId>>
where
    N: Into<usize> + Copy + std::fmt::Debug,
    W: FloatLike + std::fmt::Debug,
{
    let n = stn.variables.len();

    let mut distances = Distances::<VId, W>::new(stn.origin(), n);
    let d = &mut distances.dists;

    let mut updated = false;
    for _ in 0..n {
        updated = false;
        for (cid, c) in stn.constraints.iter().enumerate() {
            if c.active {
                let s: usize = c.c.x;
                let e: usize = c.c.y;
                let w = c.c.w;
                if d[e].forward > d[s].forward + w {
                    d[e].forward = d[s].forward + w;
                    d[e].forward_cause = Some(cid);
                    updated = true;
                }
                if d[s].backward < d[e].backward - w {
                    d[s].backward = d[e].backward - w;
                    d[s].backward_cause = Some(cid);
                    updated = true;
                }
            }
        }
        if !updated {
            // exit early if distances where not updated in this iteration
            break;
        }
    }
    if updated {
        // distances updated in the last iteration, look for negative cycle
        for (cid, c) in stn.constraints.iter().enumerate() {
            if c.active {
                let s: usize = c.c.x;
                let e: usize = c.c.y;
                let w = c.c.w;
                if d[e].forward > d[s].forward + w {
                    // found negative cycle
                    let mut cycle = vec![cid];
                    let mut current = s;
                    loop {
                        let next_constraint_id = d[current].forward_cause.expect("No cause on member of cycle");
                        if cycle.contains(&next_constraint_id) {
                            break;
                        }
                        let nc = &stn.constraints[next_constraint_id];
                        //                        println!("{:?} nc", nc);
                        //                        println!("cur: {}, next_cid: {}, ({} <= {} +?)", current, next_constraint_id, nc.c.x, nc.c.y);
                        if !nc.internal {
                            cycle.push(next_constraint_id);
                        }
                        current = nc.c.x;
                    }
                    return Result::Err(cycle);
                }
            }
        }
        panic!("No cycle found ")
    } else {
        // finished in at most n iterations
        let mut domains = IdMap::new();
        for (k, d) in distances.dists.into_iter().enumerate() {
            if let (_, Some(label)) = &stn.variables[k] {
                domains.insert(
                    *label,
                    Dom {
                        min: -d.forward,
                        max: -d.backward,
                    },
                )
            }
        }
        Result::Ok(domains)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let mut stn: STN<usize, i32> = STN::new();
        stn.add_node(1, 0, 10);
        stn.add_node(2, 0, 10);

        assert!(domains(&stn).is_ok());

        stn.record_constraint(1, 2, -15, true);

        assert!(domains(&stn).is_err());
    }
}
