use satisfy::{Container, FromContainer, Resolve, ResolveAll};
use std::sync::Arc;

struct Pool;

impl<C> FromContainer<(), C> for Pool {
    fn from_container(_: C) -> Self {
        Pool
    }
}

#[derive(Clone)]
struct RequestId(u64);

struct Handler {
    id: u64,
    pool: Arc<Pool>,
}

impl<I, C> FromContainer<I, C> for Handler
where
    C: ResolveAll<I, (RequestId, Arc<Pool>)>,
{
    fn from_container(c: C) -> Self {
        let (id, pool) = c.resolve_all();
        Handler { id: id.0, pool }
    }
}

fn resolve_from_arc<C, I>(c: Arc<C>) -> Handler
where
    C: Resolve<I, Handler>,
{
    c.resolve()
}

fn resolve_from_scope<S, I>(s: &S) -> Handler
where
    S: Resolve<I, Handler>,
{
    s.resolve()
}

#[test]
fn owned_container_satisfies_a_plain_resolve_bound() {
    let app = Arc::new(Container::new().provide(RequestId(1)));

    let handler = resolve_from_arc(app.clone());
    let pool: Arc<Pool> = app.resolve();
    assert_eq!(handler.id, 1);
    assert!(Arc::ptr_eq(&handler.pool, &pool));
}

#[test]
fn owned_scope_satisfies_a_plain_resolve_bound() {
    let app = Container::new();
    let scope = app.scope().provide(RequestId(2));

    let handler = resolve_from_scope(&scope);
    let pool: Arc<Pool> = app.resolve();
    assert_eq!(handler.id, 2);
    assert!(Arc::ptr_eq(&handler.pool, &pool));
}
