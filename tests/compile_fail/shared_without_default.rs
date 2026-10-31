// A singleton is built from a default, and `Pool` has none.
use di::Container;
use std::sync::Arc;

struct Pool;

fn main() {
    let c = Container::new();
    let _: Arc<Pool> = c.resolve();
    let _: &Pool = c.resolve();
}
