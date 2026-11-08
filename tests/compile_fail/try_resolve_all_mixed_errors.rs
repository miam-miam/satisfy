// Fallible elements of a `try_resolve_all` tuple must share one error type.
use satisfy::{Container, TryFromContainer};

struct DbErr;
struct CacheErr;

struct Db;
impl<C> TryFromContainer<(), C> for Db {
    type Error = DbErr;

    fn try_from_container(_: C) -> Result<Self, DbErr> {
        Ok(Db)
    }
}

struct Cache;
impl<C> TryFromContainer<(), C> for Cache {
    type Error = CacheErr;

    fn try_from_container(_: C) -> Result<Self, CacheErr> {
        Ok(Cache)
    }
}

fn main() {
    let _ = Container::new().try_resolve_all::<(Db, Cache), _>();
}
