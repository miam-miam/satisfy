// `override_with` only replaces a default, and `Port` has none.
use satisfy::Container;

#[derive(Clone)]
struct Port(#[allow(dead_code)] u16);

fn main() {
    let _ = Container::new().override_with(Port(8080));
}
