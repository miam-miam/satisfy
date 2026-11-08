// `Repo` needs a fallible `Db`, which needs a `Url` that is never provided.
#![allow(dead_code)]
use satisfy::{Container, Resolve, TryFromContainer, TryResolve};

#[derive(Clone)]
struct Url(String);

struct DbErr;

struct Db(Url);
impl<I, C: Resolve<I, Url>> TryFromContainer<I, C> for Db {
    type Error = DbErr;

    fn try_from_container(c: C) -> Result<Self, DbErr> {
        Ok(Db(c.resolve()))
    }
}

struct Repo(Db);
impl<I, C: TryResolve<I, Db, Error = DbErr>> TryFromContainer<I, C> for Repo {
    type Error = DbErr;

    fn try_from_container(c: C) -> Result<Self, DbErr> {
        Ok(Repo(c.try_resolve()?))
    }
}

fn main() {
    let _: Result<Repo, DbErr> = Container::new().try_resolve();
}
