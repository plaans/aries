# Development Tips


## Runtime Parameters

The Aries planner and solver will look at several environment variables to affect its behavior, notably to override default values or request additional information to be printed.

Commonly used one:

 - `ARIES_PRINT_INITIAL_PROPAGATION=true`: Will print the hierarchy before and after the initial propagation
 - `ARIES_PRINT_RAW_MODEL=true` will print the chronicles of the model before preprocessing
 - `ARIES_PRINT_MODEL=true` will print the chronicles of the model after preprocessing

While we try to list the most commonly used ones above, many more such environment variables are available. 
All such parameters are declared using the `EnvParam` type and declare environment variables whose names start with `ARIES_`. 
Looking for them can be done by grepping for `EnvParam::new` in the code base. 

## Unified Planning plugin

To pass in the unified planning plugin in development mode you should:

 - make sure that the `up-aries` module that is visible by python point to the source from the aries repository. This can be doen by e.g. adding `$HOME/aries/planning/unified/plugin` to your `PYTHON_PATH`
 - set the `ARIES_DEV` environment variable to `true`

If those conditions are verified, then at each execution, the `up-aries` module will recompile `aries` from sources and before launching the server.