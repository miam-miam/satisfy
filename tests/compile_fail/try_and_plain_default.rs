// `Db` has both a `FromContainer` and a `TryFromContainer` impl, so neither resolve can pick one.
use satisfy::{Container, FromContainer, TryFromContainer};

struct DbErr;

struct Db;

impl<C> FromContainer<(), C> for Db {
    fn from_container(_: C) -> Self {
        Db
    }
}

impl<C> TryFromContainer<(), C> for Db {
    type Error = DbErr;

    fn try_from_container(_: C) -> Result<Self, DbErr> {
        Ok(Db)
    }
}

fn main() {
    let _: Result<Db, _> = Container::new().try_resolve();
}
