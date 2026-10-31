#![expect(dead_code, clippy::disallowed_names)]

use di::{Container, FromContainer, Resolve, ResolveAll};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

// Leaf with no default: must be provided.
#[derive(Clone, Debug)]
struct Baz {
    n: u32,
}

// Local type with a default and no deps: index pinned to ().
#[derive(Clone, Debug)]
struct Bar {
    s: String,
}
impl<C> FromContainer<(), C> for Bar {
    fn from_container(_: C) -> Self {
        Bar {
            s: "default".into(),
        }
    }
}

// Several deps: one index, resolved as a tuple.
#[derive(Debug)]
struct Foo {
    bar: Bar,
    baz: Baz,
}
impl<I, C> FromContainer<I, C> for Foo
where
    C: ResolveAll<I, (Bar, Baz)>,
{
    fn from_container(c: C) -> Self {
        let (bar, baz) = c.resolve_all();
        Foo { bar, baz }
    }
}

// One dep.
#[derive(Debug)]
struct Foo2 {
    foo: Foo,
}
impl<I, C> FromContainer<I, C> for Foo2
where
    C: Resolve<I, Foo>,
{
    fn from_container(c: C) -> Self {
        Foo2 { foo: c.resolve() }
    }
}

// One instance per container: resolve `Arc<Pool>` or `&Pool` instead of `Pool`.
#[derive(Debug)]
struct Pool {
    id: u32,
}
impl<C> FromContainer<(), C> for Pool {
    fn from_container(_: C) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        Pool {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
        }
    }
}

// ---- foreign types ----
trait Database: Send + Sync {
    fn name(&self) -> String;
}
struct Postgres;
impl Database for Postgres {
    fn name(&self) -> String {
        "postgres".into()
    }
}
struct MockDb;
impl Database for MockDb {
    fn name(&self) -> String {
        "mock".into()
    }
}

impl<C> FromContainer<(), C> for Postgres {
    fn from_container(_: C) -> Self {
        Postgres
    }
}

// `Arc<dyn Database>` and `&dyn Database` resolve to one shared `Postgres`.
di::dyn_default!(dyn Database => Postgres);

// Depends on a defaulted foreign type and a provided foreign type (String).
struct Service {
    db: Arc<dyn Database>,
    label: String,
}
impl<I, C> FromContainer<I, C> for Service
where
    C: ResolveAll<I, (Arc<dyn Database>, String)>,
{
    fn from_container(c: C) -> Self {
        let (db, label) = c.resolve_all();
        Service { db, label }
    }
}

// Works with any container that can build a Service.
fn describe<C: Resolve<I, Service>, I>(c: C) -> String {
    let s: Service = c.resolve();
    format!("{} on {}", s.label, s.db.name())
}

fn main() {
    // Override a default, provide a leaf.
    let c = Container::new();
    let c = c.override_with(Bar {
        s: "overridden".into(),
    });
    let c = c.provide(Baz { n: 42 });

    let foo2: Foo2 = c.resolve();
    println!("{:?}", foo2);

    // Defaults only.
    let plain = Container::new().provide(Baz { n: 1 });
    let bar: Bar = plain.resolve();
    println!("{:?}", bar);

    // Foreign types: Arc<dyn Database> uses its default, String is provided.
    let prod = Container::new().provide(String::from("prod"));
    println!("{}", describe(&prod));

    // Same graph, database swapped out.
    let test = Container::new()
        .provide(String::from("test"))
        .override_singleton::<dyn Database, _>(Arc::new(MockDb));
    println!("{}", describe(&test));

    // Singletons: every Arc<Pool> and &Pool from one container is the same Pool.
    let shared = Container::new();
    let a: Arc<Pool> = shared.resolve();
    let b: &Pool = shared.resolve();
    println!("{:?} shared: {}", a, std::ptr::eq(&*a, b));

    // Provided values can be borrowed rather than cloned.
    let label: &String = prod.resolve();
    println!("borrowed {label}");
}
