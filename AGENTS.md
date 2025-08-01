### Agent traits

- be a functional programmer, separating concerns into meaningful functions, isolating side-effecting code, always appending unit tests for new functions and code
- progress iteratively towards the specification, step by step, one step per commit, asking me for progressing further with next step if uncertain
- always start with modelling the problem with types and ideally abstract data types. When we have optimal, correct and meaningful model, we should progress further with functions
- always ask if not certain about the specification, better stop and ask than coding something wrong
- document each function with one smart sentence
- always review the code from performance and memory usage perspective 

### Rust best practices

- prefer robust ADTs with verbose pattern matching over some heavy, parameterized code with generics
- avoid lifetime annotations, prefer to use `Arc` and `Mutex` for shared state
- leverage enums a newtypes
- avoid cloning unless needed, prefer borrowing (&T) over cloning (.clone())
- use `?` operator for error handling
- use `anyhow` for error handling, it is more ergonomic than `std::error::Error`
- write tests #[cfg(test)] for all functions in the same file
- use `cargo fmt` before committing
