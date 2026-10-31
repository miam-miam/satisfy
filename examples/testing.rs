use satisfy::{Container, FromContainer, ResolveAll};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

trait Mailer: Send + Sync {
    fn send(&self, to: &str, body: &str);
}

struct Smtp;

impl Mailer for Smtp {
    fn send(&self, to: &str, body: &str) {
        println!("SMTP to {to}: {body}");
    }
}

impl<C> FromContainer<(), C> for Smtp {
    fn from_container(_: C) -> Self {
        Smtp
    }
}

satisfy::dyn_default!(dyn Mailer => Smtp);

#[derive(Clone)]
enum Clock {
    System,
    #[cfg_attr(not(test), expect(dead_code, reason = "only tests freeze the clock"))]
    Fixed(u64),
}

impl Clock {
    fn now(&self) -> u64 {
        match self {
            Clock::System => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Clock::Fixed(secs) => *secs,
        }
    }
}

impl<C> FromContainer<(), C> for Clock {
    fn from_container(_: C) -> Self {
        Clock::System
    }
}

struct SignupService {
    mailer: Arc<dyn Mailer>,
    clock: Clock,
}

impl<I, C> FromContainer<I, C> for SignupService
where
    C: ResolveAll<I, (Arc<dyn Mailer>, Clock)>,
{
    fn from_container(c: C) -> Self {
        let (mailer, clock) = c.resolve_all();
        SignupService { mailer, clock }
    }
}

impl SignupService {
    fn sign_up(&self, email: &str) {
        let body = format!("Welcome! You signed up at {}.", self.clock.now());
        self.mailer.send(email, &body);
    }
}

fn main() {
    let container = Container::new();
    let signups: SignupService = container.resolve();
    signups.sign_up("ada@example.com");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingMailer {
        sent: Mutex<Vec<(String, String)>>,
    }

    impl Mailer for RecordingMailer {
        fn send(&self, to: &str, body: &str) {
            self.sent.lock().unwrap().push((to.into(), body.into()));
        }
    }

    #[test]
    fn sign_up_sends_a_welcome_email() {
        let mailer = Arc::new(RecordingMailer::default());
        // `override_singleton` for the mailer: `dyn Mailer` isn't `Clone`, and the test keeps a
        // handle to read what was sent. `Clock` is `Clone`, so `override_with` is enough.
        let container = Container::new()
            .override_singleton::<dyn Mailer, _>(mailer.clone())
            .override_with(Clock::Fixed(1_000));

        let signups: SignupService = container.resolve();
        signups.sign_up("ada@example.com");

        let sent = mailer.sent.lock().unwrap();
        assert_eq!(
            *sent,
            [(
                "ada@example.com".into(),
                "Welcome! You signed up at 1000.".into()
            )]
        );
    }
}
