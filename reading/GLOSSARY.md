# Rust glossary (with C comparisons)

A quick reference, not a substitute for the linked chapters in [The Book](offline-book/index.html).

| Term | Meaning | C-oriented note |
|---|---|---|
| binding | A name introduced with `let`, such as `let count = 3;` | Similar to declaring a local variable, but immutable by default. |
| mutability (`mut`) | Permission to change a binding's value or mutate through it | Rust requires mutation to be explicit in the binding/type. |
| type inference | Compiler determines a type from how a value is used | Rust infers many local types, but remains statically typed. |
| expression | Code that evaluates to a value | A block's final expression has no semicolon and becomes its value. |
| statement | An instruction that does not itself return a value | A trailing semicolon commonly turns an expression into a statement. |
| ownership | Each value has one owner; when the owner leaves scope, its value is dropped | Replaces many manual allocation/freeing patterns; compiler checks moves and lifetimes. |
| move | Transfer of ownership to a new binding or function | For many non-`Copy` values, using the old binding afterward is rejected. |
| `Copy` | Small/simple value semantics where assignment copies bits and keeps the original usable | Common for integer and boolean values; unlike moving a heap-owning `String`. |
| borrow | Temporarily access a value through a reference without taking ownership | Similar to a constrained pointer; borrow rules are checked by the compiler. |
| reference (`&T`, `&mut T`) | Borrowed access; shared references read, mutable references can mutate | `&mut T` has exclusive access while borrowed; safe Rust prevents conflicting aliases. |
| lifetime (`'a`) | A name for how long references are valid, used when the relationship needs to be expressed | Not a runtime timer; it helps prove references cannot outlive their data. |
| slice (`&[T]`, `&str`) | Borrowed view into a sequence or string | Carries a pointer and length; safe indexing checks bounds. `str` is UTF-8 text. |
| `String` | Growable, owned UTF-8 string | Heap allocated and owned; `&str` is a borrowed string slice. |
| enum | A type with a fixed set of variants, optionally carrying data | More expressive than a C enum; `match` can inspect variants and payloads. |
| `Option<T>` | `Some(value)` or `None`; represents a value that may be absent | Makes absence explicit instead of relying on a null pointer convention. |
| `Result<T, E>` | `Ok(value)` or `Err(error)`; represents success or failure | Explicit error return type; `?` propagates an error to the caller. |
| `match` | Exhaustive branching over patterns | Similar to `switch`, but checks all cases and can destructure data. |
| pattern matching | Destructure or test values using patterns in `match`, `if let`, and `let` | Combines branching with safe extraction of enum/struct fields. |
| struct | Named fields grouped into a type | Similar to a C struct, with methods defined separately in `impl` blocks. |
| trait | Shared behavior/interface that types can implement | Similar in purpose to interfaces/type classes; supports static and dynamic dispatch. |
| generic | Type or function parameterized over types, such as `Vec<T>` | Usually monomorphized at compile time, retaining static type checking. |
| crate | A Rust compilation unit/package target (library or executable) | Closest comparison is a compilation unit/library; a Cargo package can contain multiple crates. |
| module (`mod`) | Organizes code and controls name visibility | Similar to namespaces, with explicit `pub` visibility. |
| Cargo | Rust's build tool and package manager | Handles builds, dependencies, tests, and project metadata. |
| macro (`!`) | Code generation/expansion, e.g. `println!` and `vec!` | More powerful than a C preprocessor macro and checked after expansion. |
| panic | Unrecoverable program failure path | Not a normal error return; use `Result` for expected recoverable failures. |
| `unsafe` | Opt-in block for operations whose safety contract the programmer must uphold | Needed for raw pointer operations and some low-level APIs; surrounding checks still apply. |
| trait object (`dyn Trait`) | Runtime-dispatched value behind a pointer, e.g. `Box<dyn Display>` | Similar to a vtable-based interface. |
| iterator | A value that yields items, often lazily; consumed by `for`, `map`, `collect` | Standard library abstraction over sequences, with many operations checked by types. |

## Tiny syntax reminders

- `let x = 4;` immutable binding; `let mut x = 4;` mutable binding.
- `fn add(a: i32, b: i32) -> i32 { a + b }` explicit function signature; final expression is returned.
- `Vec<T>` is a growable vector; `[T; N]` is a fixed-size array; `&[T]` is a slice.
- `Option<T>` and `Result<T, E>` are enums; handle them with `match`, `if let`, or `?` where appropriate.
- `cargo run`, `cargo check`, `cargo test`, `cargo fmt`, and `cargo clippy` are common project commands.
