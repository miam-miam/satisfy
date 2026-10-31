// Resolving a type that both the scope and its parent provide is ambiguous.
use di::Container;

#[derive(Clone)]
struct Port(#[allow(dead_code)] u16);

fn main() {
    let app = Container::new().provide(Port(80));
    let scope = app.scope().provide(Port(443));
    let _: Port = scope.resolve();
}
