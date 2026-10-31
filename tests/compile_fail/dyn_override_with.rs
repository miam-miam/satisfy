// `override_with` can't take an `Arc<dyn Db>`, even with a `dyn_default!`; `override_singleton` can.
use di::{Container, FromContainer};
use std::sync::Arc;

trait Db: Send + Sync {}

struct Postgres;
impl Db for Postgres {}
impl<C> FromContainer<(), C> for Postgres {
    fn from_container(_: C) -> Self {
        Postgres
    }
}

di::dyn_default!(dyn Db => Postgres);

fn main() {
    let mock: Arc<dyn Db> = Arc::new(Postgres);
    let c = Container::new().override_with(mock);
    let _: Arc<dyn Db> = c.resolve();
}
