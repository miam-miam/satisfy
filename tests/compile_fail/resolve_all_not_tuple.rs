// `resolve_all` takes a tuple of 2 to 12 dependencies.
use satisfy::Container;

fn main() {
    let c = Container::new().provide(1u32);
    let _: u32 = c.resolve_all();
    let _: (u32,) = c.resolve_all();
}
