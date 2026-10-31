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
//! See `examples/axum` for a more complete example.

use crate::Resolve;
use axum_core::extract::{FromRef, FromRequestParts};
use http::request::Parts;
use std::convert::Infallible;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

/// Extracts a `T` resolved from the container in the router state: either an `Arc<C>` state, or
/// any state with a [`FromRef`] impl for `Arc<C>`.
///
/// `I` is the resolution path. Keep it generic on the handler just like a
/// [`Resolve`] or [`ResolveAll`](crate::ResolveAll) bound.
///
/// Destructure it as `Inject(value, _)`, or use the value through [`Deref`].
pub struct Inject<T, I>(pub T, pub PhantomData<fn() -> I>);

impl<T, I> Inject<T, I> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Container, FromContainer};
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
}
