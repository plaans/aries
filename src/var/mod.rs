mod basic_var;
mod gen_var;
mod var_array;
mod var;

pub use basic_var::BasicVar;
pub use gen_var::GenVar;
pub use gen_var::VarBool;
pub use gen_var::VarInt;
pub use var_array::VarBoolArray;
pub use var_array::VarIntArray;
pub use var::Var;