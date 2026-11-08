// `Db` only has a fallible default, so plain `resolve` can't build its singleton.
use satisfy::{Container, TryFromContainer};
use std::sync::Arc;

struct DbErr;

struct Db;
impl<C> TryFromContainer<(), C> for Db {
    type Error = DbErr;

    fn try_from_container(_: C) -> Result<Self, DbErr> {
        Ok(Db)
    }
}

fn main() {
    let _: Arc<Db> = Container::new().resolve();
}
