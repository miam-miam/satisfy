// An element of a `ResolveAll` tuple can't be resolved, reached through the by-value container.
use satisfy::{Container, FromContainer, Resolve, ResolveAll};

#[derive(Clone)]
struct Config;

struct Users;

impl<I, C> FromContainer<I, C> for Users
where
    C: Resolve<I, Config>,
{
    fn from_container(_: C) -> Self {
        Users
    }
}

fn both<C, I>(c: &C) -> (Users, Users)
where
    C: ResolveAll<I, (Users, Users)>,
{
    c.resolve_all()
}

fn main() {
    let c = Container::new();
    let _ = both(&c);
}
