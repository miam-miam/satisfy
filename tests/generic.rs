use satisfy::{Container, FromContainer, Resolve};
use std::sync::Arc;

trait Sms: Send + Sync + 'static {
    fn send(&self, to: &str) -> String;
}

struct Twilio;
impl Sms for Twilio {
    fn send(&self, to: &str) -> String {
        format!("twilio -> {to}")
    }
}
impl<C> FromContainer<(), C> for Twilio {
    fn from_container(_: C) -> Self {
        Twilio
    }
}

// No default: tests provide it.
#[derive(Clone)]
struct Recorder(&'static str);
impl Sms for Recorder {
    fn send(&self, to: &str) -> String {
        format!("{} recorded {to}", self.0)
    }
}

struct Alerts<S: Sms> {
    sms: S,
}

impl<I, C, S> FromContainer<I, C> for Alerts<S>
where
    S: Sms,
    C: Resolve<I, S>,
{
    fn from_container(c: C) -> Self {
        Alerts { sms: c.resolve() }
    }
}

#[test]
fn generic_dependency_is_built_from_its_default() {
    let alerts: Alerts<Twilio> = Container::new().resolve();

    assert_eq!(alerts.sms.send("ops"), "twilio -> ops");
}

#[test]
fn generic_dependency_is_taken_from_a_provided_value() {
    let alerts: Alerts<Recorder> = Container::new().provide(Recorder("test")).resolve();

    assert_eq!(alerts.sms.send("ops"), "test recorded ops");
}

#[test]
fn each_instantiation_gets_its_own_singleton() {
    let c = Container::new().provide(Recorder("test"));
    let real: Arc<Alerts<Twilio>> = c.resolve();
    let real_again: Arc<Alerts<Twilio>> = c.resolve();
    let fake: Arc<Alerts<Recorder>> = c.resolve();

    assert!(Arc::ptr_eq(&real, &real_again));
    assert_eq!(real.sms.send("ops"), "twilio -> ops");
    assert_eq!(fake.sms.send("ops"), "test recorded ops");
}
