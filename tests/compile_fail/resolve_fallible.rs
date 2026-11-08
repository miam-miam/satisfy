// `Db` only has a fallible default, so plain `resolve` can't build it.
use satisfy::{Container, TryFromContainer};

struct DbErr;

struct Db;
impl<C> TryFromContainer<(), C> for Db {
    type Error = DbErr;

    fn try_from_container(_: C) -> Result<Self, DbErr> {
        Ok(Db)
    }
}

fn main() {
    let _: Db = Container::new().resolve();
}
