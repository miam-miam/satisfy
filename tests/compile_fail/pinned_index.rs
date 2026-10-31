// Pinning the dependency's path to `()` only works when it's built from a default.
use di::{Container, FromContainer, Resolve};

#[derive(Clone)]
struct Port(#[allow(dead_code)] u16);

struct Server(#[allow(dead_code)] Port);
impl<C: Resolve<(), Port>> FromContainer<(), C> for Server {
    fn from_container(c: C) -> Self {
        Server(c.resolve())
    }
}

fn main() {
    let _: Server = Container::new().provide(Port(80)).resolve();
}
