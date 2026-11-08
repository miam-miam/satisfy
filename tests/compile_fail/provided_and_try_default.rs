// `Db` is provided and also has a fallible default, so `try_resolve` finds two sources.
use satisfy::{Container, TryFromContainer};

#[derive(Clone)]
struct Db;

struct DbErr;

impl<C> TryFromContainer<(), C> for Db {
    type Error = DbErr;

    fn try_from_container(_: C) -> Result<Self, DbErr> {
        Ok(Db)
    }
}

fn main() {
    let c = Container::new().provide(Db);
    let _: Result<Db, DbErr> = c.try_resolve();
}
