#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[doc = include_str!("../compile-errors.md")]
pub mod guide_compile_errors {}

#[cfg(feature = "axum")]
pub mod axum;
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
use strategy::{Skip, SkipPath};

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
    note = "give `{Self}` a `FromContainer` impl; for a `dyn Trait`, use `satisfy::dyn_default!(dyn Trait => Impl)`"
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
    /// Resolves `T`, building it and its dependencies as needed.
    ///
    /// ```
    /// # use satisfy::{Container, FromContainer, Resolve};
    /// #[derive(Clone)]
    /// struct Port(u16);
    ///
    /// struct Server {
    ///     port: u16,
    /// }
    ///
    /// impl<I, C> FromContainer<I, C> for Server
    /// where
    ///     C: Resolve<I, Port>,
    /// {
    ///     fn from_container(c: C) -> Self {
    ///         let port: Port = c.resolve();
    ///         Server { port: port.0 }
    ///     }
    /// }
    ///
    /// fn port<C, I>(c: C) -> u16
    /// where
    ///     C: Resolve<I, Server>,
    /// {
    ///     let server: Server = c.resolve();
    ///     server.port
    /// }
    ///
    /// let container = Container::new().provide(Port(8080));
    /// assert_eq!(port(&container), 8080);
    /// ```
    fn resolve(&self) -> T;
}

/// Resolves a tuple of 2 to 12 dependencies, using [`Resolve`] for each element.
///
/// Written as its own trait rather than a
/// [`Resolve`] impl so the elements may borrow from the container. For one dependency, use [`Resolve`].
/// `I` is a tuple of the elements' resolution paths, inferred (see [`strategy`] for more details).
///
/// A `C: ResolveAll<I, (A, B)>` bound also implies `C: Resolve<_, A>` and `C: Resolve<_, B>`, so
/// generic code can hand the elements to code that needs them one at a time with a single `I`.
// Also shown when an element is missing, so it can't claim `T` isn't a tuple; `Tuple`'s own
// message covers that case.
#[diagnostic::on_unimplemented(
    message = "this container can't resolve every element of `{T}`",
    label = "an element of `{T}` can't be resolved here",
    note = "each element (or something it depends on) needs `.provide(..)`, or a `FromContainer` impl whose dependencies are all resolvable"
)]
pub trait ResolveAll<I, T: sealed::Tuple>:
    Resolve<Self::P1, T::E1>
    + Resolve<Self::P2, T::E2>
    + Resolve<Self::P3, T::E3>
    + Resolve<Self::P4, T::E4>
    + Resolve<Self::P5, T::E5>
    + Resolve<Self::P6, T::E6>
    + Resolve<Self::P7, T::E7>
    + Resolve<Self::P8, T::E8>
    + Resolve<Self::P9, T::E9>
    + Resolve<Self::P10, T::E10>
    + Resolve<Self::P11, T::E11>
    + Resolve<Self::P12, T::E12>
{
    #[doc(hidden)]
    type P1;
    #[doc(hidden)]
    type P2;
    #[doc(hidden)]
    type P3;
    #[doc(hidden)]
    type P4;
    #[doc(hidden)]
    type P5;
    #[doc(hidden)]
    type P6;
    #[doc(hidden)]
    type P7;
    #[doc(hidden)]
    type P8;
    #[doc(hidden)]
    type P9;
    #[doc(hidden)]
    type P10;
    #[doc(hidden)]
    type P11;
    #[doc(hidden)]
    type P12;

    /// Resolves every element of `T`, building each one and its dependencies as needed.
    ///
    /// ```
    /// # use satisfy::{Container, FromContainer, Resolve, ResolveAll};
    /// # use std::sync::Arc;
    /// #[derive(Clone)]
    /// struct Port(u16);
    ///
    /// struct Pool;
    ///
    /// impl<C> FromContainer<(), C> for Pool {
    ///     fn from_container(_: C) -> Self {
    ///         Pool
    ///     }
    /// }
    ///
    /// struct Server {
    ///     port: u16,
    ///     pool: Arc<Pool>,
    /// }
    ///
    /// impl<I, C> FromContainer<I, C> for Server
    /// where
    ///     C: ResolveAll<I, (Port, Arc<Pool>)>,
    /// {
    ///     fn from_container(c: C) -> Self {
    ///         let (port, pool) = c.resolve_all();
    ///         Server { port: port.0, pool }
    ///     }
    /// }
    ///
    /// let container = Container::new().provide(Port(8080));
    /// let server: Server = container.resolve();
    /// let pool: Arc<Pool> = container.resolve();
    /// assert_eq!(server.port, 8080);
    /// assert!(Arc::ptr_eq(&server.pool, &pool));
    /// ```
    fn resolve_all(&self) -> T;
}

mod sealed {
    #[diagnostic::on_unimplemented(
        message = "`ResolveAll` takes a tuple of 2 to 12 dependencies, not `{Self}`",
        label = "not a tuple of 2 to 12 dependencies",
        note = "to resolve a single dependency, use `Resolve` (`.resolve()`) instead"
    )]
    pub trait Tuple {
        type E1;
        type E2;
        type E3;
        type E4;
        type E5;
        type E6;
        type E7;
        type E8;
        type E9;
        type E10;
        type E11;
        type E12;
    }
}

// Each impl names the slots it fills (`T I E P`) and the unused slots (`E P`), which resolve
// `Skip` so that every `ResolveAll` supertrait holds.
macro_rules! tuple_resolve_all {
    ($($t:ident $i:ident $e:ident $p:ident),+ $(; $($pe:ident $pp:ident),+)?) => {
        impl<$($t),+> sealed::Tuple for ($($t,)+) {
            $(type $e = $t;)+
            $($(type $pe = Skip;)+)?
        }

        impl<C, $($t, $i),+> ResolveAll<($($i,)+), ($($t,)+)> for C
        where
            $(C: Resolve<$i, $t>,)+
            C: Resolve<SkipPath, Skip>,
        {
            $(type $p = $i;)+
            $($(type $pp = SkipPath;)+)?

            fn resolve_all(&self) -> ($($t,)+) {
                ($(<C as Resolve<$i, $t>>::resolve(self),)+)
            }
        }
    };
}

tuple_resolve_all!(T1 I1 E1 P1, T2 I2 E2 P2; E3 P3, E4 P4, E5 P5, E6 P6, E7 P7, E8 P8, E9 P9, E10 P10, E11 P11, E12 P12);
tuple_resolve_all!(T1 I1 E1 P1, T2 I2 E2 P2, T3 I3 E3 P3; E4 P4, E5 P5, E6 P6, E7 P7, E8 P8, E9 P9, E10 P10, E11 P11, E12 P12);
tuple_resolve_all!(T1 I1 E1 P1, T2 I2 E2 P2, T3 I3 E3 P3, T4 I4 E4 P4; E5 P5, E6 P6, E7 P7, E8 P8, E9 P9, E10 P10, E11 P11, E12 P12);
tuple_resolve_all!(T1 I1 E1 P1, T2 I2 E2 P2, T3 I3 E3 P3, T4 I4 E4 P4, T5 I5 E5 P5; E6 P6, E7 P7, E8 P8, E9 P9, E10 P10, E11 P11, E12 P12);
tuple_resolve_all!(T1 I1 E1 P1, T2 I2 E2 P2, T3 I3 E3 P3, T4 I4 E4 P4, T5 I5 E5 P5, T6 I6 E6 P6; E7 P7, E8 P8, E9 P9, E10 P10, E11 P11, E12 P12);
tuple_resolve_all!(T1 I1 E1 P1, T2 I2 E2 P2, T3 I3 E3 P3, T4 I4 E4 P4, T5 I5 E5 P5, T6 I6 E6 P6, T7 I7 E7 P7; E8 P8, E9 P9, E10 P10, E11 P11, E12 P12);
tuple_resolve_all!(T1 I1 E1 P1, T2 I2 E2 P2, T3 I3 E3 P3, T4 I4 E4 P4, T5 I5 E5 P5, T6 I6 E6 P6, T7 I7 E7 P7, T8 I8 E8 P8; E9 P9, E10 P10, E11 P11, E12 P12);
tuple_resolve_all!(T1 I1 E1 P1, T2 I2 E2 P2, T3 I3 E3 P3, T4 I4 E4 P4, T5 I5 E5 P5, T6 I6 E6 P6, T7 I7 E7 P7, T8 I8 E8 P8, T9 I9 E9 P9; E10 P10, E11 P11, E12 P12);
tuple_resolve_all!(T1 I1 E1 P1, T2 I2 E2 P2, T3 I3 E3 P3, T4 I4 E4 P4, T5 I5 E5 P5, T6 I6 E6 P6, T7 I7 E7 P7, T8 I8 E8 P8, T9 I9 E9 P9, T10 I10 E10 P10; E11 P11, E12 P12);
tuple_resolve_all!(T1 I1 E1 P1, T2 I2 E2 P2, T3 I3 E3 P3, T4 I4 E4 P4, T5 I5 E5 P5, T6 I6 E6 P6, T7 I7 E7 P7, T8 I8 E8 P8, T9 I9 E9 P9, T10 I10 E10 P10, T11 I11 E11 P11; E12 P12);
tuple_resolve_all!(T1 I1 E1 P1, T2 I2 E2 P2, T3 I3 E3 P3, T4 I4 E4 P4, T5 I5 E5 P5, T6 I6 E6 P6, T7 I7 E7 P7, T8 I8 E8 P8, T9 I9 E9 P9, T10 I10 E10 P10, T11 I11 E11 P11, T12 I12 E12 P12);

/// Makes `Arc<dyn Trait>` and `&dyn Trait` resolve to the provided `Impl` singleton that implements [`FromContainer`].
///
/// ```
/// use satisfy::{Container, FromContainer};
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
/// satisfy::dyn_default!(dyn Db => Postgres);
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
/// # use satisfy::{Container, FromContainer};
/// # use std::sync::Arc;
/// # trait Db: Send + Sync {}
/// # struct Postgres;
/// # impl Db for Postgres {}
/// # impl<C> FromContainer<(), C> for Postgres {
/// #     fn from_container(_: C) -> Self {
/// #         Postgres
/// #     }
/// # }
/// satisfy::dyn_default!(dyn Db => Postgres);
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
