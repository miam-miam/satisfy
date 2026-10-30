//! The strategy module contains the type-level machinery for
//! resolving a type from a container.
//!
//! A type may have several ways of being resolved, but each resolve must find exactly one;
//! if two apply (such as a type that is both provided and has a default), it is a compile error.
//!
//! Resolving walks the list of provided values, skipping past each one that doesn't
//! match with a [`There`] step. There are three main strategies for resolving a type:
//!
//! 1. The type is provided directly in the container, taken with the [`Here`] marker,
//!    or borrowed as `&T` with the [`HereRef`] marker, or as `&T` from a provided `Arc<T>`
//!    with the [`HereDeref`] marker.
//! 2. The type is built from a [`FromContainer`] implementation, using the [`AutoBuild`] marker.
//! 3. The type is built once per container and shared as a singleton via `Arc<T>` or `&T`,
//!    using the [`SharedArc`] or [`SharedRef`] markers. `T` may be a `dyn Trait` given a
//!    default with [`dyn_default!`](crate::dyn_default).
//!
//! Strategies 2 and 3 apply only past the end of the list, after a [`There`] for every
//! provided value. In a [scope](Container::scope), the scope's own list ends with an
//! [`InParent`] step into the parent's list, and singletons come from the parent.
//!
//! These markers are inferred and should never be written by hand. They appear in error messages
//! as the path a resolve took: `There<AutoBuild<()>>` means "skip the one provided value, then
//! build it".

use std::marker::PhantomData;
use std::sync::Arc;

use crate::typed_list::{Cons, Nil};
use crate::{Container, FromContainer, Shared};

/// Continue trying to resolve the requested type from the next type in the stored list.
pub struct There<I>(PhantomData<I>);

/// Continue into the parent container's list, from the end of a scope's own list.
pub struct InParent<I>(PhantomData<I>);

/// Create the requested type from the type in the stored provide list.
/// Requires `T: Clone`, each instance is a separate copy.
///
/// ```
/// # use di::Container;
/// let c = Container::new().provide(String::from("prod")).provide(8080u16);
/// let label: String = c.resolve();
/// assert_eq!(label, "prod");
/// ```
pub struct Here;

/// Create a `&T` borrowed from a provided `T`.
///
/// ```
/// # use di::Container;
/// // Not `Clone`, so it can only be borrowed.
/// struct Config {
///     retries: u32,
/// }
///
/// let c = Container::new().provide(Config { retries: 3 });
/// let config: &Config = c.resolve();
/// assert_eq!(config.retries, 3);
/// ```
pub struct HereRef;

/// Create a `&T` borrowed from a provided `Arc<T>`, so a provided `Arc<dyn Trait>` can be
/// borrowed as `&dyn Trait`.
///
/// ```
/// # use di::Container;
/// use std::sync::Arc;
///
/// trait Db: Send + Sync {
///     fn name(&self) -> &str;
/// }
///
/// struct Postgres;
/// impl Db for Postgres {
///     fn name(&self) -> &str {
///         "pg"
///     }
/// }
///
/// let c = Container::new().provide(Arc::new(Postgres) as Arc<dyn Db>);
/// let db: &dyn Db = c.resolve();
/// assert_eq!(db.name(), "pg");
/// ```
pub struct HereDeref;

/// Built from [`FromContainer`], unless overridden.
///
/// ```
/// # use di::{Container, FromContainer, Resolve};
/// // `Clone` is only needed by `override_with`.
/// #[derive(Clone)]
/// struct Greeting(String);
///
/// impl<I, C: Resolve<I, String>> FromContainer<I, C> for Greeting {
///     fn from_container(c: C) -> Self {
///         let name: String = c.resolve();
///         Greeting(format!("hello {name}"))
///     }
/// }
///
/// let c = Container::new().provide(String::from("world"));
/// let greeting: Greeting = c.resolve();
/// assert_eq!(greeting.0, "hello world");
///
/// let c = Container::new()
///     .provide(String::from("world"))
///     .override_with(Greeting("hi".into()));
/// let greeting: Greeting = c.resolve();
/// assert_eq!(greeting.0, "hi");
/// ```
pub struct AutoBuild<I>(PhantomData<I>);

/// Creates a singleton per container, behind an `Arc<T>`.
///
/// ```
/// # use di::{Container, FromContainer};
/// # use std::sync::Arc;
/// struct Pool;
///
/// impl<C> FromContainer<(), C> for Pool {
///     fn from_container(_: C) -> Self {
///         Pool
///     }
/// }
///
/// let c = Container::new();
/// let a: Arc<Pool> = c.resolve();
/// let b: Arc<Pool> = c.resolve();
/// assert!(Arc::ptr_eq(&a, &b));
///
/// let other: Arc<Pool> = Container::new().resolve();
/// assert!(!Arc::ptr_eq(&a, &other));
/// ```
pub struct SharedArc<I>(PhantomData<I>);

/// Creates a singleton per container, behind a `&T`.
///
/// ```
/// # use di::{Container, FromContainer};
/// # use std::sync::Arc;
/// struct Pool;
///
/// impl<C> FromContainer<(), C> for Pool {
///     fn from_container(_: C) -> Self {
///         Pool
///     }
/// }
///
/// let c = Container::new();
/// let borrowed: &Pool = c.resolve();
/// let shared: Arc<Pool> = c.resolve();
/// let again: &Pool = c.resolve();
/// assert!(std::ptr::eq(borrowed, again));
/// assert!(std::ptr::eq(borrowed, &*shared));
/// ```
pub struct SharedRef<I>(PhantomData<I>);

/// Locate how T should be resolved.
#[diagnostic::on_unimplemented(
    message = "this container can't resolve `{T}`",
    label = "`{T}` can't be resolved here",
    note = "`{T}` needs `.provide(..)`, or a `FromContainer` impl whose dependencies are all resolvable"
)]
pub(crate) trait Locate<'c, T, I, C> {
    type Out;

    fn locate(&'c self, c: C) -> Self::Out;
}

/// Converts what `Locate` found.
///
/// Separate so `T: Clone` is checked after the path is chosen,
/// keeping a provided value that also has a default ambiguous even when it isn't `Clone`
/// as otherwise it would have been skipped for not matching trait bounds.
pub(crate) trait IntoValue<T> {
    fn into_value(self) -> T;
}

impl<T> IntoValue<T> for T {
    fn into_value(self) -> T {
        self
    }
}

impl<T: Clone> IntoValue<T> for &T {
    fn into_value(self) -> T {
        self.clone()
    }
}

// Path 1: the value was provided.
impl<'c, T: 'c, Tail, C> Locate<'c, T, Here, C> for Cons<T, Tail> {
    type Out = &'c T;

    fn locate(&'c self, _: C) -> &'c T {
        &self.head
    }
}

impl<'c, T: 'c, Tail, C> Locate<'c, &'c T, HereRef, C> for Cons<T, Tail> {
    type Out = &'c T;

    fn locate(&'c self, _: C) -> &'c T {
        &self.head
    }
}

impl<'c, T: ?Sized + 'c, Tail, C> Locate<'c, &'c T, HereDeref, C> for Cons<Arc<T>, Tail> {
    type Out = &'c T;

    fn locate(&'c self, _: C) -> &'c T {
        &self.head
    }
}

impl<'c, T, H, Tail, I, C> Locate<'c, T, There<I>, C> for Cons<H, Tail>
where
    Tail: Locate<'c, T, I, C>,
{
    type Out = Tail::Out;

    fn locate(&'c self, c: C) -> Self::Out {
        self.tail.locate(c)
    }
}

// Path 2: built from FromContainer, unless overridden.
impl<'c, T, I, R> Locate<'c, T, AutoBuild<I>, &'c Container<R>> for Nil
where
    T: FromContainer<I, &'c Container<R>>,
{
    type Out = T;

    fn locate(&'c self, c: &'c Container<R>) -> T {
        c.build::<T, I>()
    }
}

// Path 3: built once per container, then shared.
impl<'c, T, I, R> Locate<'c, Arc<T>, SharedArc<I>, &'c Container<R>> for Nil
where
    T: ?Sized + Shared<I, &'c Container<R>> + 'c,
{
    type Out = Arc<T>;

    fn locate(&'c self, c: &'c Container<R>) -> Arc<T> {
        c.singleton::<T, I>().clone()
    }
}

impl<'c, T, I, R> Locate<'c, &'c T, SharedRef<I>, &'c Container<R>> for Nil
where
    T: ?Sized + Shared<I, &'c Container<R>> + 'c,
{
    type Out = &'c T;

    fn locate(&'c self, c: &'c Container<R>) -> &'c T {
        c.singleton::<T, I>()
    }
}
