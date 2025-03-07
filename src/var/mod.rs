mod var_array;
mod basic_var;
mod var_bool;
mod var_int;
mod var;

pub use var_array::BoolArrayVariable;
pub use var_array::IntArrayVariable;
pub use basic_var::BasicVar;
pub use var_bool::VarBool;
pub use var_int::VarInt;
pub use var::Var;