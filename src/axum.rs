//! [axum](https://docs.rs/axum) integration: extract dependencies from a container held in the
//! router's state.
//!
//! ```rust,ignore
//! use satisfy::axum::Inject;
//!
//! // The one bound covers every service the handlers inject.
//! fn router<C, I>() -> Router<Arc<C>>
//! where
//!     C: ResolveAll<I, (UserService, Stats)> + Send + Sync + 'static,
//!     I: 'static,
//! {
//!     Router::new()
//!         .route("/users", get(count))
//!         .route("/users/{id}", get(greet))
//! }
//!
//! async fn greet<I>(users: Inject<UserService, I>, Path(id): Path<u64>) -> String {
//!     users.greet(id)
//! }
//!
//! router().with_state(Arc::new(container))
//! ```
//!
//! The state can also be your own type holding the container, given
//! `impl<C> FromRef<AppState<C>> for Arc<C>`.
//!
//! For a type whose default can fail, extract [`TryInject`] instead: a failed build rejects the
//! request with the type's error, which must implement [`IntoResponse`].
//!
//! See [`examples/axum`](https://github.com/miam-miam/satisfy/tree/main/examples/axum) for a more
//! complete example.

use crate::{Resolve, TryResolve};
use axum_core::extract::{FromRef, FromRequestParts};
use axum_core::response::IntoResponse;
use http::request::Parts;
use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

/// Extracts a `T` resolved from the container in the router state: either an `Arc<C>` state, or
/// any state with a [`FromRef`] impl for `Arc<C>`.
///
/// `I` is the resolution path. Keep it generic on the handler just like a
/// [`Resolve`] or [`ResolveAll`](crate::ResolveAll) bound.
///
/// Destructure it as `Inject(value, _)`, or use the value through [`Deref`]. For a type whose
/// default can fail, use [`TryInject`].
pub struct Inject<T, I>(pub T, pub PhantomData<fn() -> I>);

impl<T, I> Inject<T, I> {
    /// Returns the resolved value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

// Written by hand because derives would also require `I: Debug` / `I: Clone`.
impl<T: fmt::Debug, I> fmt::Debug for Inject<T, I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Inject").field(&self.0).finish()
    }
}

impl<T: Clone, I> Clone for Inject<T, I> {
    fn clone(&self) -> Self {
        Inject(self.0.clone(), PhantomData)
    }
}

impl<T: Copy, I> Copy for Inject<T, I> {}

// `I` is `(C, P)` so that the impl can name the container type `C`; `P` is its resolution path.
impl<S, C, P, T> FromRequestParts<S> for Inject<T, (C, P)>
where
    Arc<C>: FromRef<S>,
    C: Resolve<P, T>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(_: &mut Parts, state: &S) -> Result<Self, Infallible> {
        let container: Arc<C> = Arc::from_ref(state);
        Ok(Inject(container.resolve(), PhantomData))
    }
}

impl<T, I> Deref for Inject<T, I> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

/// Like [`Inject`], for a `T` resolved with [`TryResolve`]: a failed build rejects the request
/// with `T`'s error, which must implement [`IntoResponse`].
///
/// ```rust,ignore
/// impl IntoResponse for DbDown {
///     fn into_response(self) -> Response {
///         StatusCode::SERVICE_UNAVAILABLE.into_response()
///     }
/// }
///
/// async fn orders<I>(db: TryInject<Arc<Db>, I>) -> String {
///     db.summary()
/// }
/// ```
///
/// A generic router bounds each injected type, keeping its error generic:
/// `C: TryResolve<I, Arc<Db>>, C::Error: IntoResponse`.
pub struct TryInject<T, I>(pub T, pub PhantomData<fn() -> I>);

impl<T, I> TryInject<T, I> {
    /// Returns the resolved value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: fmt::Debug, I> fmt::Debug for TryInject<T, I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TryInject").field(&self.0).finish()
    }
}

impl<T: Clone, I> Clone for TryInject<T, I> {
    fn clone(&self) -> Self {
        TryInject(self.0.clone(), PhantomData)
    }
}

impl<T: Copy, I> Copy for TryInject<T, I> {}

impl<S, C, P, T> FromRequestParts<S> for TryInject<T, (C, P)>
where
    Arc<C>: FromRef<S>,
    C: TryResolve<P, T>,
    C::Error: IntoResponse,
    S: Send + Sync,
{
    type Rejection = C::Error;

    async fn from_request_parts(_: &mut Parts, state: &S) -> Result<Self, C::Error> {
        let container: Arc<C> = Arc::from_ref(state);
        Ok(TryInject(container.try_resolve()?, PhantomData))
    }
}

impl<T, I> Deref for TryInject<T, I> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Container, FromContainer, Resolve, TryFromContainer};
    use axum_core::response::Response;
    use http::StatusCode;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    struct Pool;

    impl<C> FromContainer<(), C> for Pool {
        fn from_container(_: C) -> Self {
            Pool
        }
    }

    fn ready<F: Future>(f: F) -> F::Output {
        match pin!(f).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(out) => out,
            Poll::Pending => panic!("extractor should not wait"),
        }
    }

    #[test]
    fn inject_resolves_from_the_router_state() {
        let state = Arc::new(Container::new());
        let (mut parts, ()) = http::Request::new(()).into_parts();

        let Ok(pool) = ready(Inject::<Arc<Pool>, _>::from_request_parts(
            &mut parts, &state,
        ));
        let shared: Arc<Pool> = state.resolve();
        assert!(Arc::ptr_eq(&pool.into_inner(), &shared));
    }

    struct AppState<C> {
        container: Arc<C>,
    }

    impl<C> FromRef<AppState<C>> for Arc<C> {
        fn from_ref(state: &AppState<C>) -> Arc<C> {
            state.container.clone()
        }
    }

    #[test]
    fn inject_resolves_from_a_container_inside_app_state() {
        let container = Arc::new(Container::new());
        let state = AppState {
            container: container.clone(),
        };
        let (mut parts, ()) = http::Request::new(()).into_parts();

        let Ok(pool) = ready(Inject::<Arc<Pool>, _>::from_request_parts(
            &mut parts, &state,
        ));
        let shared: Arc<Pool> = container.resolve();
        assert!(Arc::ptr_eq(&pool.into_inner(), &shared));
    }

    #[derive(Clone)]
    struct Url(&'static str);

    struct DbDown;

    impl IntoResponse for DbDown {
        fn into_response(self) -> Response {
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }

    struct Db;

    impl<I, C: Resolve<I, Url>> TryFromContainer<I, C> for Db {
        type Error = DbDown;

        fn try_from_container(c: C) -> Result<Self, DbDown> {
            let url: Url = c.resolve();
            if url.0.is_empty() {
                Err(DbDown)
            } else {
                Ok(Db)
            }
        }
    }

    #[test]
    fn try_inject_rejects_with_the_dependency_error() {
        let (mut parts, ()) = http::Request::new(()).into_parts();

        let down = Arc::new(Container::new().provide(Url("")));
        let Err(rejection) = ready(TryInject::<Arc<Db>, _>::from_request_parts(
            &mut parts, &down,
        )) else {
            panic!("a failing build should reject the request");
        };
        assert_eq!(
            rejection.into_response().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );

        let up = Arc::new(Container::new().provide(Url("pg://up")));
        let db = ready(TryInject::<Arc<Db>, _>::from_request_parts(&mut parts, &up));
        assert!(Arc::ptr_eq(
            &db.ok().unwrap().into_inner(),
            &up.try_resolve().ok().unwrap()
        ));
    }
}
