A crate for managing global parameters that can be set from environment variable.
The crate proposes a data-structure `EnvParam` that holds the name of an environment variable
and a string representing its default value.
The typical usage is to expose internal parameters that are not used frequently enough to appear
as command line parameters but might be used to tune the behavior of an algorithm.

```rust
use aries_env_param::EnvParam;
static MY_PARAM: EnvParam<u32> = EnvParam::new("MY_PARAM", "0");
fn main() {
  // environment variable not set, using default value "0"
  assert_eq!(MY_PARAM.get(), 0);
}
```

If the environment variable is *not* set (programmatically or in the shell) prior to the parameter's first access,
the value of the parameter will be read from the environment variable.

```rust
use aries_env_param::EnvParam;
static MY_PARAM: EnvParam<u32> = EnvParam::new("MY_PARAM", "0");
fn main() {
  std::env::set_var("MY_PARAM", "9");
  // the environment variable is set prior to the first access, and for the accessed value
  assert_eq!(MY_PARAM.get(), 9);
}
```

An `EnvParam` is only initialized once on first read. Hence any change to the environment variable
after the first access would be silently ignored.
The `EnvParam::set` provides another way to  force initialization with a given value and would panic if
the value is already initialized.

```rust
use aries_env_param::EnvParam;
static MY_PARAM: EnvParam<u32> = EnvParam::new("MY_PARAM", "0");
fn main() {
  // the environment variable is not set, default value is used
  MY_PARAM.set(10);
  assert_eq!(MY_PARAM.get(), 10);
  std::env::set_var("MY_PARAM", "999"); // set after first read, ignored
  // MY_PARAM.set(999) // would panic, since MY_PARAM is already initialized
  assert_eq!(MY_PARAM.get(), 10);
}
```
