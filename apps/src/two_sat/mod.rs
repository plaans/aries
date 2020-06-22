use aries_collections::index_map::ToIndex;

struct Implications<I> {
    n: usize,
    // An n x n matrix. The entry edges[i,j] indicates that there is a path from i to j
    edges: Vec<bool>,
    base: std::marker::PhantomData<I>,
}

impl<I: ToIndex> Implications<I> {
    pub fn new(size: usize) -> Self {
        assert!(I::first_index() <= 2); // indices start at 0 in the reresentation so a lot of space might be wasted
        Implications {
            n: size,
            edges: vec![false; size * size],
            base: std::marker::PhantomData,
        }
    }

    pub fn has_path(&self, s: I, t: I) -> bool {
        assert!(s.to_index() < self.n);
        assert!(t.to_index() < self.n);
        self.e(s.to_index(), t.to_index())
    }

    fn e(&self, a: usize, b: usize) -> bool {
        self.edges[a * self.n + b]
    }
    fn set(&mut self, a: usize, b: usize) {
        self.edges[a * self.n + b] = true
    }

    pub fn add_edge(&mut self, s: I, t: I) {
        let mut i_queue = Vec::with_capacity(16);
        let mut j_queue = Vec::with_capacity(16);

        let a = s.to_index();
        let b = t.to_index();

        if self.e(a, b) {
            // already a path between a and b, nothing to do
            return ();
        }
        self.set(a, b);

        for x in I::first_index()..self.n {
            if x == a || x == b {
                continue;
            }
            if self.e(x, a) {
                // there was a path x -> a, add the x -> b path and enqueue
                self.set(x, b);
                i_queue.push(x)
            }
            if self.e(b, x) {
                // there was a path b -> x, add the a -> x path and enqueue
                self.set(a, x);
                j_queue.push(x)
            }
        }

        /* Unoptimized version, direct adaptation from incremental Floyd-Warshall
        for &i in &i_queue {
            for &j in &j_queue {
                if i == j { continue }
                if self.e(i, j) { continue } // path already identified
                assert!(self.e(i, a) && self.e(a, j)); // should be true by construction of the i and j queue
                if self.e(i, a) && self.e(a, j) {
                    self.set(i, j)
                }
            }
        } */
        for &i in &i_queue {
            for &j in &j_queue {
                assert!(self.e(i, a) && self.e(a, j)); // should be true by construction of the i and j queue
                self.set(i, j);
            }
        }
    }
}

trait BinaryConstraint<V, D> {
    fn left_var(&self) -> V;
    fn right_var(&self) -> V;

    fn lr_propagator(&self) -> Box<dyn DirectionalPropagator<D>>;
    fn rl_propagator(&self) -> Box<dyn DirectionalPropagator<D>>;
}

trait DirectionalPropagator<D> {
    /** New domain for X based on the current domain of X and Y */
    fn restrict(&self, dom_x: D, dom_y: D) -> D;
}

#[derive(Clone, Copy)]
pub struct Dom {
    pub min: i32,
    pub max: i32,
}

/** X = Y + b */
#[derive(Clone, Copy)]
struct EqPlus<V> {
    x: V,
    y: V,
    b: i32,
}

impl<V: Copy> DirectionalPropagator<Dom> for EqPlus<V> {
    fn restrict(&self, dom_x: Dom, dom_y: Dom) -> Dom {
        Dom {
            min: i32::max(dom_y.min, dom_x.min - self.b),
            max: i32::min(dom_y.max, dom_x.max - self.b),
        }
    }
}

impl<V: 'static + Copy> BinaryConstraint<V, Dom> for EqPlus<V> {
    fn left_var(&self) -> V {
        self.x
    }
    fn right_var(&self) -> V {
        self.y
    }

    fn lr_propagator(&self) -> Box<dyn DirectionalPropagator<Dom>> {
        Box::new(*self)
        // Box::from(self as &dyn DirectionalPropagator<Dom>)
        //self as &dyn DirectionalPropagator<Dom>
    }
    fn rl_propagator(&self) -> Box<dyn DirectionalPropagator<Dom>> {
        let rev = Box::new(EqPlus {
            x: self.y,
            y: self.x,
            b: -self.b,
        });
        rev as Box<dyn DirectionalPropagator<Dom>>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base() {
        let mut g = Implications::new(20);
        println!(" {} ", g.has_path(1, 2));
        assert!(!g.has_path(1, 2));
        g.add_edge(1, 2);
        assert!(g.has_path(1, 2));
        g.add_edge(2, 3);
        assert!(g.has_path(2, 3));
        assert!(g.has_path(1, 3));
    }

    #[test]
    fn test_prop() {
        let dom_a = Dom { min: 0, max: 10 };
        let dom_b = Dom { min: 5, max: 15 };
        let c = EqPlus { x: 'a', y: 'b', b: -10 };
        let lr_prop = c.lr_propagator();
        let dom_b2 = lr_prop.restrict(dom_a, dom_b);
        assert_eq!(dom_b2.min, 10);
        assert_eq!(dom_b2.max, 15);

        let rl_prop = c.rl_propagator();
        let dom_a2 = rl_prop.restrict(dom_b, dom_a);
        assert_eq!(dom_a2.min, 0);
        assert_eq!(dom_a2.max, 5);
    }
}
