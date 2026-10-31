// A scope's `override_singleton` on a `dyn Db` also needs a `dyn_default!` to replace.
use di::Container;
use std::sync::Arc;

trait Db: Send + Sync {}

struct Mock;
impl Db for Mock {}

fn main() {
    let app = Container::new();
    let _ = app.scope().override_singleton::<dyn Db, _>(Arc::new(Mock));
}
