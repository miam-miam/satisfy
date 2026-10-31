use satisfy::{Container, FromContainer, Resolve, ResolveAll};
use std::sync::Arc;

#[derive(Clone)]
struct Config(String);

struct Pool;

impl<C> FromContainer<(), C> for Pool {
    fn from_container(_: C) -> Self {
        Pool
    }
}

struct Users(String);

impl<I, C> FromContainer<I, C> for Users
where
    C: Resolve<I, Config>,
{
    fn from_container(c: C) -> Self {
        let config: Config = c.resolve();
        Users(config.0)
    }
}

struct Stats(Arc<Pool>);

impl<I, C> FromContainer<I, C> for Stats
where
    C: Resolve<I, Arc<Pool>>,
{
    fn from_container(c: C) -> Self {
        Stats(c.resolve())
    }
}

fn users<C, J>(c: &C) -> String
where
    C: Resolve<J, Users>,
{
    c.resolve().0
}

fn stats<C, J>(c: &C) -> Arc<Pool>
where
    C: Resolve<J, Stats>,
{
    c.resolve().0
}

fn hand_off<C, I>(c: &C) -> (String, Arc<Pool>)
where
    C: ResolveAll<I, (Users, Stats, Config)>,
{
    (users(c), stats(c))
}

#[test]
fn resolve_all_bound_lets_generic_code_resolve_each_element() {
    let app = Container::new().provide(Config("hi".into()));

    // `&&app` makes `C = &Container`, keeping the by-value `Resolve` impl out of this test.
    let (name, _) = hand_off(&&app);
    assert_eq!(name, "hi");
}
