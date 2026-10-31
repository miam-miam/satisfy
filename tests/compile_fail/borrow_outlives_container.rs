// A resolved `&T` borrows the container, so it can't outlive it.
use satisfy::Container;

struct Config;

fn main() {
    let config: &Config;
    {
        let c = Container::new().provide(Config);
        config = c.resolve();
    }
    let _ = config;
}
