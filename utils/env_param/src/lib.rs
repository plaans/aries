#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

use std::str::FromStr;

/// Holds a parameter, with a default value and overridable progrmatically or through an environment variable.
pub struct EnvParam<T> {
    value: once_cell::sync::OnceCell<T>,
    env: &'static str,
    default: &'static str,
}

impl<T> EnvParam<T> {
    /// Creates a new parameter that will be initialized from the environment variable `env` or set
    /// `default` if the environment variable is not set.
    pub const fn new(env: &'static str, default: &'static str) -> EnvParam<T> {
        EnvParam {
            value: once_cell::sync::OnceCell::new(),
            env,
            default,
        }
    }
}

impl<T: FromStr> EnvParam<T> {
    fn read_default(&self) -> T {
        match T::from_str(self.default) {
            Ok(v) => v,
            Err(_) => {
                eprintln!(
                    "[env_param] ERROR {}: could not parse the default value \"{}\".",
                    self.env, self.default
                );
                panic!("[env_param] {}: invalid default value \"{}\".", self.env, self.default)
            }
        }
    }

    /// Returns the value of the parameter. On the first call, the value will be read from
    /// the declared environment variable. If it is not set or has an invalid value, the
    /// default value will be used.
    ///
    /// # Panic
    /// The method will panic if the parameter cannot be parsed from the default value.
    /// A warning will be printed if the environment variable is set but cannot be parsed.
    pub fn get(&self) -> T
    where
        T: Copy,
    {
        *self.get_ref()
    }

    /// Returns the value of the parameter. On the first call, the value will be read from
    /// the declared environment variable. If it is not set or has an invalid value, the
    /// default value will be used.
    ///
    /// # Panic
    /// The method will panic if the parameter cannot be parsed from the default value.
    /// A warning will be printed if the environment variable is set but cannot be parsed.
    pub fn get_ref(&self) -> &T {
        let read = || match std::env::var(self.env) {
            Result::Ok(param) => match T::from_str(&param) {
                Result::Ok(value) => value,
                Result::Err(_) => {
                    eprintln!("[env_param] WARNING: could not parse the value \"{}\" for environment variable \"{}\". Using default: \"{}\" ", &param, self.env, self.default);
                    self.read_default()
                }
            },
            Result::Err(std::env::VarError::NotPresent) => self.read_default(),
            Result::Err(err) => {
                eprintln!(
                    "[env_param] {}: {}. Using default: \"{}\" ",
                    self.env, err, self.default
                );
                self.read_default()
            }
        };
        self.value.get_or_init(read)
    }

    /// Set the parameter to the given value.
    ///
    /// # Panic
    /// Panics if the parameters has already been set, which typically means it has already been read.
    pub fn set(&self, value: T) {
        if self.value.set(value).is_err() {
            panic!(
                "Parameter {} is already initialized (i.e. was previously accessed).",
                self.env
            );
        }
    }
}
