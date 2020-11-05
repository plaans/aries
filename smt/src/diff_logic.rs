use super::*;
use aries_sat::SatLiteral;

trait DifferenceLogic<Atom, Variable, Value>: Theory<Atom> {
    fn diff_below(&mut self, a: Variable, b: Variable, value: Value) -> Atom;
    fn diff_strictly_below(&mut self, a: Variable, b: Variable, value: Value) -> Atom;

    fn leq<Lit: SatLiteral>(&mut self, a: Variable, b: Variable) -> Lit
    where
        Self: SMTProblem<Lit, Atom>,
        Value: num_traits::Zero,
    {
        let atom = self.diff_below(b, a, Value::zero());
        self.literal_of(atom)
    }
}
