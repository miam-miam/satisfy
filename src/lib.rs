//! Compile-time checked dependency injection. Start with [`Container`].
//!
//! ```
//! use di::{Container, FromContainer, Resolve};
//!
//! #[derive(Clone)]
//! struct Config {
//!     name: String,
//! }
//!
//! struct Greeter {
//!     config: Config,
//! }
//! impl<I, C: Resolve<I, Config>> FromContainer<I, C> for Greeter {
//!     fn from_container(c: C) -> Self {
//!         Greeter { config: c.resolve() }
//!     }
//! }
//!
//! let container = Container::new().provide(Config { name: "world".into() });
//! let greeter: Greeter = container.resolve();
//! assert_eq!(greeter.config.name, "world");
//! ```

#[doc = include_str!("../compile-errors.md")]
pub mod guide_compile_errors {}

mod container;
mod once_map;
mod scope;
pub mod strategy;
mod typed_list;

#[doc(inline)]
pub use container::Container;

#[doc(inline)]
pub use scope::Scoped;

use std::sync::Arc;

/// A default way to build `Self` from a container.
/// `I` is the resolution path of the dependencies (see [`strategy`]): keep it generic and pass it
/// to the `Resolve` bound.
// TODO: Document this
// `I` comes first so that a downstream crate can give a default to a
// foreign type by using a local marker as `I` (orphan rules require a
// local type before any uncovered type parameter such as `C`).
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no default (`FromContainer` impl)",
    label = "no default for `{Self}`",
    note = "`{Self}` needs `.provide(..)` or a `FromContainer` impl; `.override_with(..)` only replaces an existing default"
)]
pub trait FromContainer<I, C>: Sized + 'static {
    fn from_container(c: C) -> Self;
}

/// Builds the singleton behind `Arc<Self>` and `&Self`.
///
/// Every type with a [`FromContainer`] impl has this too. Its singleton is built the same way as a
/// plain `Self`, so [`override_with`](Container::override_with) applies to it as well.
/// A `dyn Trait` can't be returned by value, so it has no `FromContainer` impl; give it one of
/// these with [`dyn_default!`].
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no shared instance",
    label = "no shared instance for `{Self}`",
    note = "give `{Self}` a `FromContainer` impl; for a `dyn Trait`, use `di::dyn_default!(dyn Trait => Impl)`"
)]
pub trait Shared<I, C> {
    /// `Self` with its lifetimes widened to `'static`, which the singleton is stored and keyed as.
    type Stored: ?Sized + Send + Sync + 'static;

    fn build_shared(c: C) -> Arc<Self::Stored>;

    // Shorten the lifetime to match the container.
    fn from_stored(stored: &Arc<Self::Stored>) -> &Arc<Self>;
}

// The `T: FromContainer` bound keeps provided values out, so their `&T` stays a plain borrow
// and doesn't also match the singleton path.
#[diagnostic::do_not_recommend]
impl<T, I, J, C> Shared<(I, J), C> for T
where
    T: FromContainer<J, C> + Send + Sync,
    C: Resolve<I, T>,
{
    type Stored = T;

    fn build_shared(c: C) -> Arc<T> {
        Arc::new(c.resolve())
    }

    fn from_stored(stored: &Arc<T>) -> &Arc<T> {
        stored
    }
}

/// Main trait for resolving a type from a container.
///
/// `I` is the resolution path, inferred and never written by hand (see [`strategy`] for more details).
#[diagnostic::on_unimplemented(
    message = "this container can't resolve `{T}`",
    label = "`{T}` can't be resolved here",
    note = "`{T}` (or something it depends on) needs `.provide(..)`, or a `FromContainer` impl whose dependencies are all resolvable"
)]
pub trait Resolve<I, T> {
    fn resolve(&self) -> T;
}

/// Resolves a tuple of 2 to 12 dependencies, using [`Resolve`] for each element.
///
/// Written as its own trait rather than a
/// [`Resolve`] impl so the elements may borrow from the container. For one dependency, use [`Resolve`].
/// `I` is a tuple of the elements' resolution paths, inferred (see [`strategy`] for more details).
// Matches `Tuple`'s message, as both bounds fail together for a non-tuple.
#[diagnostic::on_unimplemented(
    message = "`ResolveAll` takes a tuple of 2 to 12 dependencies, not `{T}`",
    label = "not a tuple of 2 to 12 dependencies",
    note = "to resolve a single dependency, use `Resolve` (`.resolve()`) instead"
)]
pub trait ResolveAll<I, T: sealed::Tuple> {
    fn resolve_all(&self) -> T;
}

mod sealed {
    #[diagnostic::on_unimplemented(
        message = "`ResolveAll` takes a tuple of 2 to 12 dependencies, not `{Self}`",
        label = "not a tuple of 2 to 12 dependencies",
        note = "to resolve a single dependency, use `Resolve` (`.resolve()`) instead"
    )]
    pub trait Tuple {}
}

macro_rules! tuple_resolve_all {
    ($($t:ident $i:ident),+) => {
        impl<$($t),+> sealed::Tuple for ($($t,)+) {}

        impl<C, $($t, $i),+> ResolveAll<($($i,)+), ($($t,)+)> for C
        where
            $(C: Resolve<$i, $t>),+
        {
            fn resolve_all(&self) -> ($($t,)+) {
                ($(<C as Resolve<$i, $t>>::resolve(self),)+)
            }
        }
    };
}

tuple_resolve_all!(T1 I1, T2 I2);
tuple_resolve_all!(T1 I1, T2 I2, T3 I3);
tuple_resolve_all!(T1 I1, T2 I2, T3 I3, T4 I4);
tuple_resolve_all!(T1 I1, T2 I2, T3 I3, T4 I4, T5 I5);
tuple_resolve_all!(T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6);
tuple_resolve_all!(T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7);
tuple_resolve_all!(T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8);
tuple_resolve_all!(T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9);
tuple_resolve_all!(T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9, T10 I10);
tuple_resolve_all!(T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9, T10 I10, T11 I11);
tuple_resolve_all!(T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9, T10 I10, T11 I11, T12 I12);

/// Makes `Arc<dyn Trait>` and `&dyn Trait` resolve to the provided `Impl` singleton that implements [`FromContainer`].
///
/// ```
/// use di::{Container, FromContainer};
/// use std::sync::Arc;
///
/// trait Db: Send + Sync {}
///
/// struct Postgres;
/// impl Db for Postgres {}
///
/// impl<C> FromContainer<(), C> for Postgres {
///     fn from_container(_: C) -> Self {
///         Postgres
///     }
/// }
///
/// di::dyn_default!(dyn Db => Postgres);
///
/// let c = Container::new();
/// let db: Arc<dyn Db> = c.resolve();
/// let borrowed: &dyn Db = c.resolve();
/// let pg: Arc<Postgres> = c.resolve();
/// assert!(std::ptr::addr_eq(&*db, borrowed));
/// assert!(std::ptr::addr_eq(&*db, &*pg));
/// ```
///
/// To swap in another implementation, override the singleton with
/// [`override_singleton`](Container::override_singleton). An `Arc<dyn Trait>` passed to
/// [`override_with`](Container::override_with) instead competes with the default, so resolving
/// `Arc<dyn Trait>` becomes ambiguous.
///
/// ```
/// # use di::{Container, FromContainer};
/// # use std::sync::Arc;
/// # trait Db: Send + Sync {}
/// # struct Postgres;
/// # impl Db for Postgres {}
/// # impl<C> FromContainer<(), C> for Postgres {
/// #     fn from_container(_: C) -> Self {
/// #         Postgres
/// #     }
/// # }
/// di::dyn_default!(dyn Db => Postgres);
///
/// struct Mock;
/// impl Db for Mock {}
///
/// let mock: Arc<dyn Db> = Arc::new(Mock);
/// let c = Container::new().override_singleton::<dyn Db, _>(mock.clone());
/// let db: Arc<dyn Db> = c.resolve();
/// let borrowed: &dyn Db = c.resolve();
/// assert!(Arc::ptr_eq(&db, &mock));
/// assert!(std::ptr::addr_eq(borrowed, &*mock));
/// ```
#[macro_export]
macro_rules! dyn_default {
    (dyn $trait:path => $impl:ty) => {
        // For every lifetime, so a `&'c dyn Trait` bound (which means `dyn Trait + 'c`) resolves.
        impl<'a, I, C> $crate::Shared<I, C> for dyn $trait + 'a
        where
            C: $crate::Resolve<I, ::std::sync::Arc<$impl>>,
        {
            type Stored = dyn $trait;

            fn build_shared(c: C) -> ::std::sync::Arc<dyn $trait> {
                <C as $crate::Resolve<I, ::std::sync::Arc<$impl>>>::resolve(&c)
            }

            fn from_stored(stored: &::std::sync::Arc<dyn $trait>) -> &::std::sync::Arc<Self> {
                stored
            }
        }
    };
}
