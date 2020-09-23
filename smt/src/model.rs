pub enum Literal<Atom> {
    Pos(Atom),
    Neg(Atom),
}

pub struct Clause<Atom, Meta = ()> {
    internal: Vec<Literal<Atom>>,
    meta: Meta,
}

#[cfg(test)]
mod test {
    use std::ops::{Add, Sub};

    enum IDL_Atom<BVar, IVar, ICst> {
        Bool(BVar),
        DiffUB { b: IVar, a: IVar, ub: ICst },
        AbsUB { b: IVar, ub: ICst },
    }

    /// Upper bound the `b - a` difference
    struct DiffUB<IVar, ICst> {
        a: IVar,
        b: IVar,
        diff_upper_bound: ICst,
    }

    struct AbsUB<IVar, ICst> {
        a: IVar,
        upper_bound: ICst,
    }

    #[derive(Copy, Clone)]
    struct IVar(u32);
    struct IFreePoint {
        var: IVar,
        shift: i32,
    }
    impl From<IVar> for IFreePoint {
        fn from(var: IVar) -> Self {
            IFreePoint { var, shift: 0 }
        }
    }

    impl std::ops::Add<i32> for IFreePoint {
        type Output = IFreePoint;

        fn add(self, rhs: i32) -> Self::Output {
            let x: IFreePoint = self.into();
            IFreePoint {
                var: x.var,
                shift: x.shift + rhs,
            }
        }
    }
    impl std::ops::Add<i32> for IVar {
        type Output = IFreePoint;

        fn add(self, rhs: i32) -> Self::Output {
            IFreePoint { var: self, shift: rhs }
        }
    }

    impl std::ops::Sub<i32> for IFreePoint {
        type Output = IFreePoint;

        fn sub(self, rhs: i32) -> Self::Output {
            self + (-rhs)
        }
    }

    fn leq<T1: Into<IFreePoint>, T2: Into<IFreePoint>>(lhs: T1, rhs: T2) -> DiffUB<IVar, i32> {
        let lhs: IFreePoint = lhs.into();
        let rhs: IFreePoint = rhs.into();
        DiffUB {
            b: lhs.var,
            a: rhs.var,
            diff_upper_bound: rhs.shift - lhs.shift,
        }
    }
    fn lt<T1: Into<IFreePoint>, T2: Into<IFreePoint>>(lhs: T1, rhs: T2) -> DiffUB<IVar, i32> {
        leq(lhs.into(), rhs.into() - 1)
    }

    #[test]
    fn test() {
        let v = IVar(0);

        let x = v + 3;
        leq(v + 2, x - 3);
        assert!(false);
        println!("AAA")
    }
}
