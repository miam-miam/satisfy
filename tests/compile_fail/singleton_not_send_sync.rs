// Singletons are shared across threads, so a `!Send` type can't be one.
use satisfy::{Container, FromContainer};
use std::rc::Rc;
use std::sync::Arc;

struct Cache(#[allow(dead_code)] Rc<u32>);
impl<C> FromContainer<(), C> for Cache {
    fn from_container(_: C) -> Self {
        Cache(Rc::new(0))
    }
}

fn main() {
    let _: Arc<Cache> = Container::new().resolve();
}
