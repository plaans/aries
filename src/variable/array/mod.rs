mod array_variable;
mod generic_array_variable;
mod shared_bool_array_variable;
mod shared_int_array_variable;

pub use array_variable::ArrayVariable;
pub use generic_array_variable::ArrayIntVariable;
pub use generic_array_variable::ArrayBoolVariable;
pub use shared_bool_array_variable::SharedArrayBoolVariable;
pub use shared_int_array_variable::SharedArrayIntVariable;