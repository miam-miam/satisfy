// `Port` is provided and also has a default, so resolving it is ambiguous, even though it isn't `Clone`.
use di::{Container, FromContainer};

struct Port(#[allow(dead_code)] u16);
impl<C> FromContainer<(), C> for Port {
    fn from_container(_: C) -> Self {
        Port(80)
    }
}

fn main() {
    let c = Container::new().provide(Port(443));
    let _: Port = c.resolve();
}
