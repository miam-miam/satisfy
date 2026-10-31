// A bound on the container itself only covers values that don't borrow from it.
use satisfy::{Container, Resolve};

#[derive(Clone)]
struct Config;

fn config<'a, C, I>(c: &'a C) -> &'a Config
where
    C: Resolve<I, &'a Config>,
{
    c.resolve()
}

fn main() {
    let c = Container::new().provide(Config);
    let _ = config(&c);
}
