#[macro_export]
macro_rules! print_info {
    ($v:expr, $($arg:tt)*) => {
        if $v == true {
            print!("\x1b[95m[INFO]\x1b[0m ");
            println!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! print_expr {
    ($v:expr, $($arg:tt)*) => {
        if $v == true {
            print!("\x1b[96m[EXPR]\x1b[0m ");
            println!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! print_assign {
    ($v:expr, $($arg:tt)*) => {
        if $v == true {
            print!("\x1b[92m[ASSIGN]\x1b[0m ");
            println!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! print_warn {
    ($v:expr, $($arg:tt)*) => {
        if $v == true {
            print!("\x1b[93m[WARN]\x1b[0m ");
            println!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! print_debug {
    ($v:expr, $($arg:tt)*) => {
        if $v == true {
            print!("\x1b[94m[DEBUG]\x1b[0m ");
            println!($($arg)*);
        }
    };
}
