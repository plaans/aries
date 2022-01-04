# Variables


A variable in aries core is represented by the `VarRef` type.
A `VarRef` type is the ID of a variable.







## The Zero variable

Among the possible instances of a `VarRef`, the `VarRef::ZERO` constant is a reserved value that is always equal to `0`.


# Creating Variables

## Manually creating variables.

IT is possible to manually create variables by using `VarRef::from_u32(i)` where `i` an unsigned integer on 32 bits (`u32`). This is typically non what you want to do but allows:

 - converting an index to a variable (this what is used internally in the implementation of aries)
 - create variable instances for testing purposes.

The first variable is reserved as the `VarRef::ZERO` value.

```rust
assert_eq!(VarRef::from_u32(0), VarRef::ZERO);
``` 



## Creating variables in a model

In a typical use case, one would want to create a contiguous set of variable and have a data structure hold their metadata. 

The `Domains` type provides such a container. It allows:

 - requesting the creation of a new variable with a given domain (lower and upper bounds). The `Domains` type will allocate a memory slot to track the metadata of the variable and return a unique ID for this variable in the form of a `VarRef`.[^Note that the ID is only unique to the instance of the `Domains` it was created from.]
 - Various methods to update and query the current domain of any variable that was ever created in it.