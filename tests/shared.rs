use satisfy::{Container, FromContainer, Resolve};
use std::sync::Arc;

trait Db: Send + Sync {
    fn name(&self) -> String;
}
#[derive(Clone)]
struct Postgres(String);
impl Db for Postgres {
    fn name(&self) -> String {
        self.0.clone()
    }
}
impl<C> FromContainer<(), C> for Postgres {
    fn from_container(_: C) -> Self {
        Postgres("pg".into())
    }
}
struct Mock;
impl Db for Mock {
    fn name(&self) -> String {
        "mock".into()
    }
}

satisfy::dyn_default!(dyn Db => Postgres);

// `&'c dyn Db` here means `&'c (dyn Db + 'c)`, not the `'static` object the singleton is stored as.
struct Repo {
    db_name: String,
}

impl<'c, I, C> FromContainer<I, C> for Repo
where
    C: Resolve<I, &'c dyn Db>,
{
    fn from_container(c: C) -> Self {
        let db: &dyn Db = c.resolve();
        Repo { db_name: db.name() }
    }
}

#[test]
fn dependency_on_short_lived_dyn_ref_uses_the_default() {
    let c = Container::new();
    let repo: Repo = c.resolve();
    assert_eq!(repo.db_name, "pg");
}

#[test]
fn dependency_on_short_lived_dyn_ref_sees_scope_override() {
    let app = Container::new();
    let scope = app.scope().override_singleton::<dyn Db, _>(Arc::new(Mock));
    let repo: Repo = scope.resolve();
    assert_eq!(repo.db_name, "mock");
}

#[test]
fn override_with_feeds_the_singleton() {
    let c = Container::new().override_with(Postgres("replica".into()));
    let a: Arc<dyn Db> = c.resolve();
    let b: Arc<dyn Db> = c.resolve();
    let pg: Arc<Postgres> = c.resolve();
    assert_eq!(a.name(), "replica");
    assert_eq!(pg.name(), "replica");
    assert!(Arc::ptr_eq(&a, &b));
}

#[test]
fn provided_clone_type_borrow_is_unambiguous() {
    let c = Container::new().provide(String::from("x"));
    let s: &String = c.resolve();
    let y: &String = c.resolve();
    assert_eq!(s, "x");
    assert!(std::ptr::addr_eq(s, y));
}
