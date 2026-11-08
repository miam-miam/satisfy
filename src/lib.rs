#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

#[doc = include_str!("../compile-errors.md")]
pub mod _compile_errors {}

#[cfg(feature = "axum")]
pub mod axum;
mod container;
mod once_map;
mod resolve_all;
mod scope;
mod shared;
pub mod strategy;
pub mod typed_list;

#[doc(inline)]
pub use container::Container;

#[doc(inline)]
pub use scope::Scoped;

#[doc(inline)]
pub use resolve_all::{ResolveAll, TryResolveAll, TryResolveIn};
#[doc(hidden)]
pub use shared::{HasShared, Shared, TryShared};

use std::convert::Infallible;
use strategy::{Fallible, Plain};

/// A default way to build `Self` from a container.
///
/// `I` is the resolution path of the dependencies (see [`strategy`]): keep it generic and pass it
/// to the `Resolve` bound.
///
/// A type with no dependencies pins `I` to `()`. A type with dependencies stays generic over `I`
/// and `C`, and lists what it needs as [`Resolve`] or [`ResolveAll`] bounds on `C`:
///
/// ```
/// # use satisfy::{Container, FromContainer, ResolveAll};
///
/// #[derive(Clone)]
/// struct ApiKey(String);
///
/// struct RetryPolicy {
///     attempts: u8,
/// }
///
/// impl<C> FromContainer<(), C> for RetryPolicy {
///     fn from_container(_: C) -> Self {
///         RetryPolicy { attempts: 3 }
///     }
/// }
///
/// struct WeatherClient {
///     key: String,
///     attempts: u8,
/// }
///
/// impl<I, C> FromContainer<I, C> for WeatherClient
/// where
///     C: ResolveAll<I, (ApiKey, RetryPolicy)>,
/// {
///     fn from_container(c: C) -> Self {
///         let (key, retry) = c.resolve_all();
///         WeatherClient { key: key.0, attempts: retry.attempts }
///     }
/// }
///
/// // `ApiKey` has no default, so it must be provided; the rest are built on demand.
/// let container = Container::new().provide(ApiKey("secret".into()));
/// let client: WeatherClient = container.resolve();
///
/// assert_eq!(client.key, "secret");
/// assert_eq!(client.attempts, 3);
/// ```
///
/// To give a default to a type from another crate, wrap `I` in a local marker so the orphan rules
/// allow the impl. If two crates do this for the same type, resolving it becomes ambiguous.
///
/// ```
/// # use satisfy::{Container, FromContainer, Resolve};
/// # use std::marker::PhantomData;
/// # use std::path::PathBuf;
/// struct Mine<I>(PhantomData<I>);
///
/// #[derive(Clone)]
/// struct AppName(&'static str);
///
/// impl<I, C> FromContainer<Mine<I>, C> for PathBuf
/// where
///     C: Resolve<I, AppName>,
/// {
///     fn from_container(c: C) -> Self {
///         let name: AppName = c.resolve();
///         PathBuf::from("/var/lib").join(name.0)
///     }
/// }
///
/// let data_dir: PathBuf = Container::new().provide(AppName("notes")).resolve();
///
/// assert_eq!(data_dir, PathBuf::from("/var/lib/notes"));
/// ```
// `I` comes first so that a downstream crate can give a default to a
// foreign type by using a local marker as `I` (orphan rules require a
// local type before any uncovered type parameter such as `C`).
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no default (`FromContainer` impl)",
    label = "no default for `{Self}`",
    note = "`{Self}` needs `.provide(..)` or a `FromContainer` impl; `.override_with(..)` only replaces an existing default",
    note = "if `{Self}` has a `TryFromContainer` impl, resolve it with `try_resolve` (`TryResolve`) instead"
)]
pub trait FromContainer<I, C>: Sized + 'static {
    /// Builds `Self`, resolving its dependencies from `c`.
    fn from_container(c: C) -> Self;
}

/// A default way to build `Self` from a container that can fail, such as one that reads a file.
///
/// Resolve it with [`try_resolve`](Container::try_resolve), which returns `Result<Self, Error>`.
/// It is written exactly like [`FromContainer`], with dependencies listed as
/// [`TryResolve`] or [`TryResolveAll`] bounds.
///
/// ```
/// # use satisfy::{Container, Resolve, TryFromContainer};
/// # use std::sync::Arc;
/// #[derive(Clone)]
/// struct CertPem(&'static str);
///
/// #[derive(Debug, PartialEq)]
/// struct BadCert;
///
/// struct TlsCert {
///     subject: String,
/// }
///
/// impl<I, C> TryFromContainer<I, C> for TlsCert
/// where
///     C: Resolve<I, CertPem>,
/// {
///     type Error = BadCert;
///
///     fn try_from_container(c: C) -> Result<Self, BadCert> {
///         let pem: CertPem = c.resolve();
///         let subject = pem.0.strip_prefix("CN=").ok_or(BadCert)?;
///         Ok(TlsCert { subject: subject.into() })
///     }
/// }
///
/// let good = Container::new().provide(CertPem("CN=example.org"));
/// let cert: Arc<TlsCert> = good.try_resolve().unwrap();
///
/// assert_eq!(cert.subject, "example.org");
///
/// let bad = Container::new().provide(CertPem("garbage"));
/// let err = bad.try_resolve::<TlsCert, _>().err();
///
/// assert_eq!(err, Some(BadCert));
/// ```
///
/// A type with both a `FromContainer` and a `TryFromContainer` impl can't be resolved.
pub trait TryFromContainer<I, C>: Sized + 'static {
    /// What building `Self` fails with.
    type Error: 'static;

    /// Builds `Self`, resolving its dependencies from `c`.
    fn try_from_container(c: C) -> Result<Self, Self::Error>;
}

/// Resolves a type that may have a [`TryFromContainer`] impl.
///
/// Anything [`Resolve`] can resolve can be resolved this way too, with `Error = Infallible`, so
/// generic code with a `C: Resolve<I, T>` bound can call `try_resolve` or hand `C` to code that
/// needs `TryResolve`. A type built from [`TryFromContainer`] (directly, or shared as `Arc<T>`
/// or `&T`) has that impl's `Error`.
///
/// Generic code only sees the error through its bound, since it depends on how `C` resolves `T`.
/// To use it as a concrete type, for example with `?`, either name it,
/// `C: TryResolve<I, T, Error = DbError>`, or require only that it converts into your own error,
/// `C: TryResolve<I, T>, AppError: From<C::Error>`.
///
/// When a dependency is missing, the error names the type you asked for rather than the missing
/// one; resolve its dependencies one at a time to find it.
#[diagnostic::on_unimplemented(
    message = "this container can't try to resolve `{T}`",
    label = "`{T}` can't be resolved here",
    note = "`{T}` (or something it depends on) needs `.provide(..)`, or a `TryFromContainer` impl whose dependencies are all resolvable; for an `Arc<dyn Trait>` or `&dyn Trait`, `satisfy::try_dyn_default!`",
    note = "this error names the type you asked for, not the missing dependency; resolve `{T}`'s dependencies one at a time to find it"
)]
pub trait TryResolve<I, T>: sealed::Resolver {
    /// What resolving `T` fails with.
    type Error;

    /// Resolves `T`, building it and its dependencies as needed.
    ///
    /// ```
    /// # use satisfy::{Container, Resolve, TryFromContainer, TryResolve};
    /// #[derive(Clone)]
    /// struct ShardCount(u32);
    ///
    /// #[derive(Debug)]
    /// struct NoShards;
    ///
    /// struct ShardMap {
    ///     shards: u32,
    /// }
    ///
    /// impl<I, C> TryFromContainer<I, C> for ShardMap
    /// where
    ///     C: Resolve<I, ShardCount>,
    /// {
    ///     type Error = NoShards;
    ///
    ///     fn try_from_container(c: C) -> Result<Self, NoShards> {
    ///         let count: ShardCount = c.resolve();
    ///         match count.0 {
    ///             0 => Err(NoShards),
    ///             shards => Ok(ShardMap { shards }),
    ///         }
    ///     }
    /// }
    ///
    /// fn shards<C, I>(c: C) -> Result<u32, NoShards>
    /// where
    ///     C: TryResolve<I, ShardMap, Error = NoShards>,
    /// {
    ///     let map = c.try_resolve()?;
    ///     Ok(map.shards)
    /// }
    ///
    /// assert_eq!(shards(&Container::new().provide(ShardCount(4))).unwrap(), 4);
    /// assert!(shards(&Container::new().provide(ShardCount(0))).is_err());
    /// ```
    fn try_resolve(&self) -> Result<T, Self::Error>;
}

// Every resolver that can `resolve` a type can `try_resolve` it, including in generic code that
// only has a `Resolve` bound. Fallible defaults are added per resolver under `Fallible`.
impl<C, I, T> TryResolve<Plain<I>, T> for C
where
    C: Resolve<I, T>,
{
    type Error = Infallible;

    fn try_resolve(&self) -> Result<T, Infallible> {
        Ok(self.resolve())
    }
}

/// Overrides that replace a default.
/// i.e. the type has a [`FromContainer`] or [`TryFromContainer`] impl.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no default this container can build",
    label = "no default for `{Self}` to replace",
    note = "`.override_with(..)` only replaces an existing default: `{Self}` needs a `FromContainer` or `TryFromContainer` impl, or use `.provide(..)` instead",
    note = "if `{Self}` has one, something it depends on can't be resolved here; resolve `{Self}` directly to find what's missing"
)]
pub trait HasDefault<I, C> {}

impl<T: FromContainer<I, C>, I, C> HasDefault<Plain<I>, C> for T {}

impl<T: TryFromContainer<I, C>, I, C> HasDefault<Fallible<I>, C> for T {}

/// Main trait for resolving a type from a container.
///
/// `I` is the resolution path, inferred and never written by hand (see [`strategy`] for more details).
#[diagnostic::on_unimplemented(
    message = "this container can't resolve `{T}`",
    label = "`{T}` can't be resolved here",
    note = "`{T}` (or something it depends on) needs `.provide(..)`, or a `FromContainer` impl whose dependencies are all resolvable"
)]
pub trait Resolve<I, T>: sealed::Resolver {
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
    ///
    /// assert_eq!(port(&container), 8080);
    /// ```
    fn resolve(&self) -> T;
}

mod sealed {
    use crate::{Container, Scoped};

    pub trait Resolver {}

    impl<TList> Resolver for Container<TList> {}
    impl<TList> Resolver for &Container<TList> {}
    impl<Parent, TList> Resolver for Scoped<'_, Parent, TList> {}
    impl<Parent, TList> Resolver for &Scoped<'_, Parent, TList> {}
}
