use axum::Router;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use satisfy::axum::Inject;
use satisfy::{Container, FromContainer, Resolve, ResolveAll};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Config {
    addr: String,
    greeting: String,
}

// Resolved as `Arc<UserTable>`, so every request shares one instance.
#[derive(Default)]
struct UserTable {
    names: Mutex<HashMap<u64, String>>,
}

impl<C> FromContainer<(), C> for UserTable {
    fn from_container(_: C) -> Self {
        UserTable::default()
    }
}

struct UserService {
    table: Arc<UserTable>,
    greeting: String,
}

impl<I, C> FromContainer<I, C> for UserService
where
    C: ResolveAll<I, (Arc<UserTable>, Config)>,
{
    fn from_container(c: C) -> Self {
        let (table, config) = c.resolve_all();
        UserService {
            table,
            greeting: config.greeting,
        }
    }
}

impl UserService {
    fn greet(&self, id: u64) -> Option<String> {
        let names = self.table.names.lock().unwrap();
        names
            .get(&id)
            .map(|name| format!("{}, {name}", self.greeting))
    }

    fn rename(&self, id: u64, name: String) {
        self.table.names.lock().unwrap().insert(id, name);
    }
}

struct Stats {
    table: Arc<UserTable>,
}

impl<I, C> FromContainer<I, C> for Stats
where
    C: Resolve<I, Arc<UserTable>>,
{
    fn from_container(c: C) -> Self {
        Stats { table: c.resolve() }
    }
}

impl Stats {
    fn user_count(&self) -> usize {
        self.table.names.lock().unwrap().len()
    }
}

// Generic over the container so the router never names its type. The one `ResolveAll` bound
// covers every service the handlers inject.
fn router<C, I>() -> Router<Arc<C>>
where
    C: ResolveAll<I, (UserService, Stats)> + Send + Sync + 'static,
    I: 'static,
{
    Router::new()
        .route("/users", get(count))
        .route("/users/{id}", get(greet).put(rename))
}

async fn count<I>(stats: Inject<Stats, I>) -> String {
    stats.user_count().to_string()
}

async fn greet<I>(
    users: Inject<UserService, I>,
    Path(id): Path<u64>,
) -> Result<String, StatusCode> {
    users.greet(id).ok_or(StatusCode::NOT_FOUND)
}

async fn rename<I>(Inject(users, _): Inject<UserService, I>, Path(id): Path<u64>, name: String) {
    users.rename(id, name);
}

#[tokio::main]
async fn main() {
    let config = Config {
        addr: "127.0.0.1:3000".into(),
        greeting: "Hello".into(),
    };
    let listener = tokio::net::TcpListener::bind(&config.addr).await.unwrap();
    let container = Container::new().provide(config);

    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router().with_state(Arc::new(container)))
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    fn config() -> Config {
        Config {
            addr: String::new(),
            greeting: "Hi".into(),
        }
    }

    fn app() -> Router {
        router().with_state(Arc::new(Container::new().provide(config())))
    }

    // The body on success, otherwise the status.
    async fn send(app: &Router, request: Request<Body>) -> Result<String, StatusCode> {
        let response = app.clone().oneshot(request).await.unwrap();
        if !response.status().is_success() {
            return Err(response.status());
        }
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        Ok(String::from_utf8(bytes.to_vec()).unwrap())
    }

    async fn get(app: &Router, uri: &str) -> Result<String, StatusCode> {
        send(app, Request::get(uri).body(Body::empty()).unwrap()).await
    }

    async fn put(app: &Router, uri: &str, body: &str) {
        let request = Request::put(uri).body(Body::from(body.to_owned())).unwrap();
        assert_eq!(send(app, request).await, Ok(String::new()));
    }

    #[tokio::test]
    async fn requests_share_the_user_table_singleton() {
        let app = app();

        put(&app, "/users/1", "Ada").await;
        assert_eq!(get(&app, "/users/1").await, Ok("Hi, Ada".into()));
        assert_eq!(get(&app, "/users").await, Ok("1".into()));
    }

    #[tokio::test]
    async fn unknown_user_is_not_found() {
        assert_eq!(get(&app(), "/users/7").await, Err(StatusCode::NOT_FOUND));
    }

    #[tokio::test]
    async fn user_table_can_be_replaced_with_a_fixture() {
        let fixture = UserTable::default();
        fixture.names.lock().unwrap().insert(2, "Grace".into());
        let container = Container::new()
            .provide(config())
            .override_singleton(Arc::new(fixture));
        let app = router().with_state(Arc::new(container));

        assert_eq!(get(&app, "/users/2").await, Ok("Hi, Grace".into()));
    }
}
