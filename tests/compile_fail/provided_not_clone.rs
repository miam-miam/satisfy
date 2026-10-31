// A provided value that isn't `Clone` can only be borrowed, not resolved by value.
use di::Container;

struct Config;

fn main() {
    let c = Container::new().provide(Config);
    let _: Config = c.resolve();
}
