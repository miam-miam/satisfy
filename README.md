Satisfy
==================
[<img alt="github" src="https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/miam-miam/satisfy)
[<img alt="crates.io" src="https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust" height="20">](https://crates.io/crates/satisfy)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/satisfy)

Satisfy is a zero-dependency trait based dependency injection container whose dependency graph is checked at compile
time. If a dependency is missing, or could come from two places, your code won't compile.

You [`provide`] the values that have no sensible default, such as configuration. Other types implement the [
`FromContainer`] trait and are built on demand from their dependencies.

<sup>Satisfy currently supports rustc 1.85 and above.</sup>

[`provide`]: https://docs.rs/satisfy/latest/satisfy/struct.Container.html#method.provide
[`FromContainer`]: https://docs.rs/satisfy/latest/satisfy/trait.FromContainer.html

## Examples

```rust
use satisfy::{Container, FromContainer, Resolve};
use std::sync::Arc;

#[derive(Clone)]
struct Config {
    db_url: String,
}

struct Pool {
    url: String,
}

impl<I, C> FromContainer<I, C> for Pool
where
    C: Resolve<I, Config>
{
    fn from_container(c: C) -> Self {
        let config: Config = c.resolve();
        Pool { url: config.db_url }
    }
}

struct UserRepo {
    pool: Arc<Pool>,
}

// Arcs are singletons, built once per container. Each `UserRepo` gets a clone of the same Arc.
impl<I, C> FromContainer<I, C> for UserRepo
where
    C: Resolve<I, Arc<Pool>>
{
    fn from_container(c: C) -> Self {
        UserRepo { pool: c.resolve() }
    }
}

let container = Container::new().provide(Config { db_url: "postgres://localhost".into() });

// Each `UserRepo` is new, but they share one `Pool`.
let a: UserRepo = container.resolve();
let b: UserRepo = container.resolve();

assert!(Arc::ptr_eq(&a.pool, &b.pool));
assert_eq!(a.pool.url, "postgres://localhost");
```

The type you ask for determines what you get:

- `T`: built fresh on each call, or a clone of the provided value.
- `&T`: a borrow of the provided value or of the Arc singleton.
- `Arc<T>`: the singleton, built once per container.

Without the `provide(Config { .. })` call, this fails to compile:

```text
error[E0277]: this container can't resolve `Arc<Pool>`
```

The [compile errors guide] explains the less obvious errors.

[compile errors guide]: https://docs.rs/satisfy/latest/satisfy/guide_compile_errors/index.html

### Overrides

[`override_with`] replaces a type's default with a fixed value, such as a mock in tests. [`override_singleton`]
replaces only the singleton behind `Arc<T>` and `&T`. It doesn't need `Clone`, so it also works for a `dyn Trait` given
a default with [`dyn_default!`].

```rust
use satisfy::{Container, FromContainer};
use std::sync::Arc;

trait Mailer: Send + Sync {
    fn host(&self) -> &str;
}

struct Smtp;
impl Mailer for Smtp {
    fn host(&self) -> &str {
        "smtp.example.com"
    }
}

impl<C> FromContainer<(), C> for Smtp {
    fn from_container(_: C) -> Self {
        Smtp
    }
}

satisfy::dyn_default!(dyn Mailer => Smtp);

struct Fake;
impl Mailer for Fake {
    fn host(&self) -> &str {
        "localhost"
    }
}

let prod = Container::new();
let test = Container::new().override_singleton::<dyn Mailer, _ > (Arc::new(Fake));

let mailer: Arc<dyn Mailer> = prod.resolve();
assert_eq!(mailer.host(), "smtp.example.com");
let mailer: Arc<dyn Mailer> = test.resolve();
assert_eq!(mailer.host(), "localhost");
```

[`override_with`]: https://docs.rs/satisfy/latest/satisfy/struct.Container.html#method.override_with
[`override_singleton`]: https://docs.rs/satisfy/latest/satisfy/struct.Container.html#method.override_singleton
[`dyn_default!`]: https://docs.rs/satisfy/latest/satisfy/macro.dyn_default.html

### Scopes

Use [`scope`] to create a child container, for example one per request. It can provide its own values and overrides, and
shares its parent's singletons. It can't provide a type the parent already provides.

You can also use [`ResolveAll`] to take several dependencies at once.

```rust
use satisfy::{Container, FromContainer, ResolveAll};
use std::sync::Arc;

#[derive(Clone)]
struct RequestId(u64);

struct Pool;
impl<C> FromContainer<(), C> for Pool {
    fn from_container(_: C) -> Self {
        Pool
    }
}

struct Handler {
    id: RequestId,
    pool: Arc<Pool>,
}

impl<I, C> FromContainer<I, C> for Handler
where
    C: ResolveAll<I, (RequestId, Arc<Pool>)>
{
    fn from_container(c: C) -> Self {
        let (id, pool) = c.resolve_all();
        Handler { id, pool }
    }
}

let app = Container::new();

let first: Handler = app.scope().provide(RequestId(1)).resolve();
let second: Handler = app.scope().provide(RequestId(2)).resolve();
assert_eq!((first.id.0, second.id.0), (1, 2));
assert!(Arc::ptr_eq(&first.pool, &second.pool));
```

[`scope`]: https://docs.rs/satisfy/latest/satisfy/struct.Container.html#method.scope
[`ResolveAll`]: https://docs.rs/satisfy/latest/satisfy/trait.ResolveAll.html

#### License

<sup>
Licensed under either of <a href="https://github.com/miam-miam/satisfy/blob/main/LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="https://github.com/miam-miam/satisfy/blob/main/LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
