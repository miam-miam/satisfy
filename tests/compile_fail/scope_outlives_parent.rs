// A scope borrows its parent container, so it can't outlive it.
use satisfy::Container;

fn main() {
    let scope;
    {
        let app = Container::new().provide(1u32);
        scope = app.scope();
    }
    let _: u32 = scope.resolve();
}
