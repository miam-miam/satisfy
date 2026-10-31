// `Port` has no default and neither the scope nor its parent provides it.
use satisfy::Container;
use std::sync::Arc;

struct Port(#[allow(dead_code)] u16);

fn main() {
    let app = Container::new();
    let scope = app.scope();
    let _: Port = scope.resolve();
    let _: Arc<Port> = scope.resolve();
}
