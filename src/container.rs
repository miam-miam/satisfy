//! The [`Container`] and its child [`Scoped`] containers.
// ^ Technically the scoped container is in a separate module,
// but we re-export it from here in the docs.

use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::once_map::{OnceMap, TypeKey, sorted_by_name};
use crate::scope::Scoped;
use crate::strategy::{IntoValue, Locate};
use crate::typed_list::{Cons, Nil, ScopeEnd};
use crate::{FromContainer, Resolve, ResolveAll, Shared};

/// A dependency injection container whose dependency graph is checked at compile time.
///
/// Give the container only the values that have no sensible default, such as configuration
/// or external handles, with [`provide`](Self::provide). Every other type should implement
/// [`FromContainer`], allowing it to be built on demand with everything it depends on.
///
/// A container is usually built once at start-up.
///
/// # Getting started
///
/// ```
/// use satisfy::{Container, FromContainer, Resolve};
///
/// // No sensible default, so it should be provided.
/// #[derive(Clone)]
/// struct GreeterConfig {
///     greeting: String,
/// }
///
/// // Built from the container, so it has a `FromContainer` impl.
/// struct Greeter {
///     config: GreeterConfig,
/// }
///
/// impl<I, C> FromContainer<I, C> for Greeter
/// where
///     C: Resolve<I, GreeterConfig>,
/// {
///     fn from_container(c: C) -> Self {
///         Greeter { config: c.resolve() }
///     }
/// }
///
/// let container = Container::new().provide(GreeterConfig { greeting: "hello".into() });
/// let greeter: Greeter = container.resolve();
///
/// assert_eq!(greeter.config.greeting, "hello");
/// ```
///
/// Use [`resolve`](Self::resolve) for one type and [`resolve_all`](Self::resolve_all) for a
/// tuple of them. A [`FromContainer`] impl with several dependencies takes them through
/// [`ResolveAll`].
///
/// # Owned, borrowed and singleton values
///
/// The type you ask for decides what you get:
///
/// - `T`: a value of its own, built on each call, or a clone of the provided one.
/// - `&T`: a borrow of the provided value, or of a singleton instance.
/// - `Arc<T>`: the singleton instance, built once per container, such as a connection pool.
///
/// ```
/// # use satisfy::{Container, FromContainer, Resolve};
/// # use std::sync::Arc;
/// // Not `Clone`, so it can only be borrowed.
/// struct DbConfig {
///     url: String,
/// }
///
/// struct Pool {
///     url: String,
/// }
///
/// // Borrow DbConfig from the container lifetime 'c
/// impl<'c, I, C> FromContainer<I, C> for Pool
/// where
///     C: Resolve<I, &'c DbConfig>,
/// {
///     fn from_container(c: C) -> Self {
///         let config = c.resolve();
///         Pool { url: config.url.clone() }
///     }
/// }
///
/// let container = Container::new().provide(DbConfig { url: "postgres://localhost".into() });
/// let config: &DbConfig = container.resolve();
///
/// assert_eq!(config.url, "postgres://localhost");
///
/// let a: Arc<Pool> = container.resolve();
/// let b: Arc<Pool> = container.resolve();
/// let c: &Pool = container.resolve();
///
/// assert!(Arc::ptr_eq(&a, &b));
/// assert!(std::ptr::eq(c, &*a));
/// ```
///
/// # Writing code against any container
///
/// The container's type grows with every [`provide`](Self::provide) (`Container<Cons<Config, Nil>>`,
/// see [`typed_list`](crate::typed_list)). It can be named, but it is usually simpler to write code
/// against `C: Resolve<I, T>` and pass `&container`.
/// Code that holds the container itself, such as an `Arc<C>` in shared state, can use the same
/// bound for values that don't borrow from it.
///
/// # Overriding defaults in tests
///
/// [`override_with`](Self::override_with) replaces a default with another value of the same
/// type, and [`override_singleton`](Self::override_singleton) replaces the singleton instance.
/// The rest of the dependency graph is unchanged and other containers are unaffected.
///
/// ```
/// # use satisfy::{Container, FromContainer, Resolve};
/// #[derive(Clone)]
/// struct Mailer {
///     host: String,
/// }
///
/// impl<C> FromContainer<(), C> for Mailer {
///     fn from_container(_: C) -> Self {
///         Mailer { host: "smtp.example.com".into() }
///     }
/// }
///
/// fn mail_host<C, I>(c: C) -> String
/// where
///     C: Resolve<I, Mailer>,
/// {
///     let mailer: Mailer = c.resolve();
///     mailer.host
/// }
///
/// let prod = Container::new();
/// let test = Container::new().override_with(Mailer { host: "localhost:1025".into() });
///
/// assert_eq!(mail_host(&prod), "smtp.example.com");
/// assert_eq!(mail_host(&test), "localhost:1025");
/// ```
///
/// To swap in a different type rather than a different value, make the dependent type generic over
/// a trait. There is no `dyn` or `Arc`, and the fake needs no default; it is just provided.
///
/// ```
/// # use satisfy::{Container, FromContainer, Resolve};
/// trait PaymentGateway {
///     fn charge(&self, pence: u32) -> bool;
/// }
///
/// struct Stripe;
/// impl PaymentGateway for Stripe {
///     fn charge(&self, _: u32) -> bool {
///         true
///     }
/// }
///
/// impl<C> FromContainer<(), C> for Stripe {
///     fn from_container(_: C) -> Self {
///         Stripe
///     }
/// }
///
/// #[derive(Clone)]
/// struct AlwaysDecline;
/// impl PaymentGateway for AlwaysDecline {
///     fn charge(&self, _: u32) -> bool {
///         false
///     }
/// }
///
/// struct Checkout<G> {
///     gateway: G,
/// }
///
/// impl<I, C, G> FromContainer<I, C> for Checkout<G>
/// where
///     G: PaymentGateway + 'static,
///     C: Resolve<I, G>,
/// {
///     fn from_container(c: C) -> Self {
///         Checkout { gateway: c.resolve() }
///     }
/// }
///
/// let live: Checkout<Stripe> = Container::new().resolve();
/// let declined: Checkout<AlwaysDecline> = Container::new().provide(AlwaysDecline).resolve();
///
/// assert!(live.gateway.charge(1999));
/// assert!(!declined.gateway.charge(1999));
/// ```
///
/// The generic parameter spreads to every type that holds a `Checkout`. To avoid that, use
/// [`dyn_default!`](crate::dyn_default) and resolve an `Arc<dyn PaymentGateway>` instead, at the
/// cost of dynamic dispatch.
///
/// # Scopes
///
/// [`scope`](Self::scope) makes a child container, such as one per request, that adds its own
/// values and overrides but shares this container's singletons.
///
/// # Compile-time checks
///
/// Each type is resolved in exactly one way (see [`strategy`](crate::strategy)). A dependency
/// that is missing, or that could come from two places, is a compile error rather than a
/// panic:
///
/// ```compile_fail,E0277
/// # use satisfy::Container;
/// struct Config;
///
/// // Never provided, and no `FromContainer` impl.
/// let config: Config = Container::new().resolve();
/// ```
///
/// Types built from `FromContainer` must be `'static`. The container is `Send` and `Sync`
/// when its provided values are; overridden and shared types must be `Send + Sync`
/// themselves.
pub struct Container<TList> {
    provided: TList,
    overrides: HashMap<TypeKey, Arc<dyn Any + Send + Sync>>,
    singletons: OnceMap<TypeKey, Box<dyn Any + Send + Sync>>,
}

impl Container<Nil> {
    /// Creates a container with nothing provided or overridden.
    #[must_use]
    pub fn new() -> Self {
        Container {
            provided: Nil,
            overrides: HashMap::new(),
            singletons: OnceMap::new(),
        }
    }
}

impl Default for Container<Nil> {
    fn default() -> Self {
        Self::new()
    }
}

impl<TList> Container<TList> {
    /// Provide `T` so that it can be resolved.
    ///
    /// For types with no [`FromContainer`] impl only. Use [`override_with`](Self::override_with)
    /// if you want to replace a [`FromContainer`] impl.
    ///
    /// ```
    /// # use satisfy::Container;
    /// #[derive(Clone)]
    /// struct Currency(&'static str);
    ///
    /// let c = Container::new().provide(Currency("EUR")).provide(2u8);
    /// let currency: Currency = c.resolve();
    /// let decimals: &u8 = c.resolve();
    ///
    /// assert_eq!((currency.0, *decimals), ("EUR", 2));
    /// ```
    #[must_use]
    pub fn provide<T>(self, value: T) -> Container<Cons<T, TList>> {
        Container {
            provided: Cons {
                head: value,
                tail: self.provided,
            },
            overrides: self.overrides,
            singletons: self.singletons,
        }
    }

    /// Override a type that has a [`FromContainer`] impl.
    ///
    /// If the type doesn't have a [`FromContainer`] impl, use [`provide`](Self::provide) instead.
    ///
    /// A plain `T` resolves to a clone of `value`. The singleton behind `Arc<T>` and `&T` is
    /// built from a clone too, unless [`override_singleton`](Self::override_singleton) sets it.
    ///
    /// ```
    /// # use satisfy::{Container, FromContainer};
    /// #[derive(Clone)]
    /// struct TaxRate(f32);
    ///
    /// impl<C> FromContainer<(), C> for TaxRate {
    ///     fn from_container(_: C) -> Self {
    ///         TaxRate(0.2)
    ///     }
    /// }
    ///
    /// let exempt = Container::new().override_with(TaxRate(0.0));
    /// let rate: TaxRate = exempt.resolve();
    ///
    /// assert_eq!(rate.0, 0.0);
    /// ```
    #[must_use]
    pub fn override_with<T, I>(mut self, value: T) -> Self
    where
        T: for<'c> FromContainer<I, &'c Self> + Clone + Send + Sync,
    {
        // A factory rather than the bare value, so `AutoBuild` doesn't require `T: Clone`.
        let factory: Box<dyn Fn() -> T + Send + Sync> = Box::new(move || value.clone());
        self.overrides.insert(TypeKey::of::<T>(), Arc::new(factory));
        self
    }

    /// Sets the singleton instance used for this container.
    ///
    /// Unlike [`override_with`](Self::override_with), `T` doesn't need to implement [`Clone`],
    /// and may be a `dyn Trait` given a default with [`dyn_default!`](crate::dyn_default).
    ///
    /// Only `Arc<T>` and `&T` resolve to this instance. A plain `T` is still built, from
    /// [`override_with`](Self::override_with)'s value if one is set, otherwise from its
    /// [`FromContainer`] impl.
    ///
    /// ```
    /// # use satisfy::{Container, FromContainer};
    /// # use std::sync::Arc;
    /// # use std::sync::atomic::{AtomicU64, Ordering};
    /// struct Counter(AtomicU64);
    ///
    /// impl<C> FromContainer<(), C> for Counter {
    ///     fn from_container(_: C) -> Self {
    ///         Counter(AtomicU64::new(0))
    ///     }
    /// }
    ///
    /// let seeded = Arc::new(Counter(AtomicU64::new(100)));
    /// let c = Container::new().override_singleton(seeded.clone());
    /// let counter: &Counter = c.resolve();
    /// counter.0.fetch_add(1, Ordering::Relaxed);
    ///
    /// assert_eq!(seeded.0.load(Ordering::Relaxed), 101);
    /// ```
    #[must_use]
    pub fn override_singleton<T, I>(mut self, value: Arc<T>) -> Self
    where
        T: ?Sized + for<'c> Shared<I, &'c Self, Stored = T> + Send + Sync + 'static,
    {
        self.singletons.set(TypeKey::of::<T>(), Box::new(value));
        self
    }

    /// Resolves a single dependency, which may borrow from the container.
    /// See [`Resolve`] for details.
    // Inherent method to remove the need for users to call
    // resolve on the reference type of the container.
    pub fn resolve<'c, T, I>(&'c self) -> T
    where
        &'c Self: Resolve<I, T>,
    {
        <&'c Self as Resolve<I, T>>::resolve(&self)
    }

    /// Resolves a tuple of dependencies, each of which may borrow from the container.
    /// See [`ResolveAll`] for details.
    // Inherent method to remove the need for users to call
    // resolve on the reference type of the container.
    pub fn resolve_all<'c, T, I>(&'c self) -> T
    where
        T: crate::sealed::Tuple,
        &'c Self: ResolveAll<I, T>,
    {
        <&'c Self as ResolveAll<I, T>>::resolve_all(&self)
    }

    /// Makes a child container, such as one per request, that borrows this one.
    ///
    /// The scope resolves its own provided values and overrides first
    /// before checking the parent's values.
    ///
    /// It uses the parent's singletons, unless the scope sets one with
    /// [`Scoped::override_singleton`] which overrides the singleton used for that scope only.
    ///
    /// ```
    /// # use satisfy::{Container, FromContainer, ResolveAll};
    /// # use std::sync::Arc;
    /// // Shared by every tenant, so built once in the parent.
    /// struct FontCache;
    ///
    /// impl<C> FromContainer<(), C> for FontCache {
    ///     fn from_container(_: C) -> Self {
    ///         FontCache
    ///     }
    /// }
    ///
    /// // Not `Clone`: borrowed from whichever scope provided it.
    /// struct Tenant {
    ///     name: String,
    /// }
    ///
    /// struct InvoiceRenderer {
    ///     header: String,
    ///     fonts: Arc<FontCache>,
    /// }
    ///
    /// impl<'c, I, C> FromContainer<I, C> for InvoiceRenderer
    /// where
    ///     C: ResolveAll<I, (&'c Tenant, Arc<FontCache>)>,
    /// {
    ///     fn from_container(c: C) -> Self {
    ///         let (tenant, fonts) = c.resolve_all();
    ///         InvoiceRenderer { header: format!("Invoice for {}", tenant.name), fonts }
    ///     }
    /// }
    ///
    /// let app = Container::new();
    /// let acme = app.scope().provide(Tenant { name: "Acme".into() });
    /// let globex = app.scope().provide(Tenant { name: "Globex".into() });
    ///
    /// let for_acme: InvoiceRenderer = acme.resolve();
    /// let for_globex: InvoiceRenderer = globex.resolve();
    ///
    /// assert_eq!(for_acme.header, "Invoice for Acme");
    /// assert_eq!(for_globex.header, "Invoice for Globex");
    /// assert!(Arc::ptr_eq(&for_acme.fonts, &for_globex.fonts));
    /// ```
    #[must_use]
    pub fn scope(&self) -> Scoped<'_, TList, ScopeEnd> {
        Scoped::new(self)
    }

    pub(crate) fn provided(&self) -> &TList {
        &self.provided
    }

    pub(crate) fn override_for(&self, key: TypeKey) -> Option<&Arc<dyn Any + Send + Sync>> {
        self.overrides.get(&key)
    }

    pub(crate) fn build<'c, T, I>(&'c self) -> T
    where
        T: FromContainer<I, &'c Self>,
    {
        if let Some(f) = self.overrides.get(&TypeKey::of::<T>()) {
            // Keyed by TypeId::of::<T>() in override_with, so this can't fail.
            let f = f
                .downcast_ref::<Box<dyn Fn() -> T + Send + Sync>>()
                .unwrap();
            return f();
        }
        T::from_container(self)
    }

    pub(crate) fn singleton<'c, T, I>(&'c self) -> &'c Arc<T>
    where
        T: ?Sized + Shared<I, &'c Self> + 'c,
    {
        let entry = self.singletons.get_or_init(TypeKey::of::<T::Stored>(), || {
            Box::new(T::build_shared(self))
        });
        T::from_stored((**entry).downcast_ref().unwrap())
    }
}

/// Lists the provided values, the types of the overrides and singletons built so far.
impl<TList: fmt::Debug> fmt::Debug for Container<TList> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Container")
            .field("provided", &self.provided)
            .field(
                "overrides",
                &sorted_by_name(self.overrides.keys().copied().collect()),
            )
            .field(
                "singletons",
                &sorted_by_name(self.singletons.initialised_keys()),
            )
            .finish()
    }
}

// Implemented for `&'c Container` only, so a resolved `&T` can borrow from the container for `'c`.
impl<'c, TList, T, I> Resolve<I, T> for &'c Container<TList>
where
    TList: Locate<'c, T, I, &'c Container<TList>>,
    TList::Out: IntoValue<T>,
{
    fn resolve(&self) -> T {
        let c: &'c Container<TList> = self;
        c.provided.locate(c).into_value()
    }
}

/// Resolves values that don't borrow from the container, so code holding it by value (such as
/// in an `Arc`) can bound `C: Resolve<I, T>` instead of `for<'c> &'c C: Resolve<I, T>`.
// Hidden from diagnostics, so errors keep pointing at the `&Container` impl's unmet requirement.
#[diagnostic::do_not_recommend]
impl<TList, T, I> Resolve<I, T> for Container<TList>
where
    for<'c> &'c Container<TList>: Resolve<I, T>,
{
    fn resolve(&self) -> T {
        <&Container<TList> as Resolve<I, T>>::resolve(&self)
    }
}

#[cfg(test)]
mod tests {
    use super::Container;

    fn assert_send_sync<T: Send + Sync>(_: &T) {}

    #[test]
    fn container_and_scope_are_send_and_sync() {
        let container = Container::new().provide(1u32);
        assert_send_sync(&container);
        assert_send_sync(&container.scope().provide(String::new()));
    }

    #[test]
    fn scope_debug_flattens_provided_values_and_shows_parent() {
        let container = Container::new().provide(1u32);
        let scope = container.scope().provide("a").provide(2u8);
        assert_eq!(
            format!("{scope:?}"),
            r#"Scoped { parent: Container { provided: [1], overrides: [], singletons: [] }, provided: [2, "a"], overrides: [], singleton_overrides: [] }"#,
        );
    }
}
