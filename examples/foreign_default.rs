use satisfy::{Container, FromContainer, Resolve};
use std::marker::PhantomData;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

// The orphan rules need a local `I` to implement `FromContainer` for std types. If two crates
// do this for the same type, resolving it becomes ambiguous.
struct Local<I>(PhantomData<I>);

impl<C> FromContainer<Local<()>, C> for Duration {
    fn from_container(_: C) -> Self {
        Duration::from_secs(30)
    }
}

#[derive(Clone)]
struct Port(u16);

impl<I, C> FromContainer<Local<I>, C> for SocketAddr
where
    C: Resolve<I, Port>,
{
    fn from_container(c: C) -> Self {
        let port: Port = c.resolve();
        SocketAddr::from((Ipv4Addr::LOCALHOST, port.0))
    }
}

fn main() {
    let container = Container::new().provide(Port(8080));
    let addr: SocketAddr = container.resolve();
    let timeout: Duration = container.resolve();
    println!("{addr} with a {timeout:?} timeout");

    let container = Container::new()
        .provide(Port(8080))
        .override_with(Duration::from_secs(5));
    let timeout: Duration = container.resolve();
    println!("overridden to {timeout:?}");
}
