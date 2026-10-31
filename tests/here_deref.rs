use satisfy::{Container, FromContainer, Resolve};
use std::sync::Arc;

trait Db: Send + Sync {
    fn name(&self) -> &'static str;
}

struct Mock;
impl Db for Mock {
    fn name(&self) -> &'static str {
        "mock"
    }
}

struct Repo {
    db_name: &'static str,
}
impl<'c, I, C> FromContainer<I, C> for Repo
where
    C: Resolve<I, &'c dyn Db>,
{
    fn from_container(c: C) -> Self {
        let db: &dyn Db = c.resolve();
        Repo { db_name: db.name() }
    }
}

#[test]
fn scope_borrows_its_own_provided_arc() {
    let app = Container::new();
    let scope = app.scope().provide(Arc::new(Mock) as Arc<dyn Db>);
    let repo: Repo = scope.resolve();
    assert_eq!(repo.db_name, "mock");
}
