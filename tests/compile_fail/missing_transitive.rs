// `App` needs `Server`, which needs a `Port` that is never provided.
use satisfy::{Container, FromContainer, Resolve};

struct Port(#[allow(dead_code)] u16);

struct Server(#[allow(dead_code)] Port);
impl<I, C: Resolve<I, Port>> FromContainer<I, C> for Server {
    fn from_container(c: C) -> Self {
        Server(c.resolve())
    }
}

struct App(#[allow(dead_code)] Server);
impl<I, C: Resolve<I, Server>> FromContainer<I, C> for App {
    fn from_container(c: C) -> Self {
        App(c.resolve())
    }
}

fn main() {
    let _: App = Container::new().resolve();
}
