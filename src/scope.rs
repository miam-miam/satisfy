use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::once_map::{TypeKey, sorted_by_name};
use crate::strategy::{AutoBuild, InParent, IntoValue, Locate, SharedArc, SharedRef};
use crate::typed_list::{Cons, Nil, ScopeEnd};
use crate::{Container, FromContainer, Resolve, ResolveAll, Shared};

/// A child of a [`Container`], made with [`Container::scope`], allowing request based overrides.
///
/// It resolves its own provided values and overrides first,
/// before checking the parent's values.
///
/// It uses the parent's singletons, unless the scope sets one with
/// [`override_singleton`](Self::override_singleton) which overrides the singleton used for
/// that scope only.
///
/// ```
/// # use di::{Container, FromContainer};
/// use std::sync::Arc;
///
/// struct Pool {
///     url: String,
/// }
///
/// impl<C> FromContainer<(), C> for Pool {
///     fn from_container(_: C) -> Self {
///         Pool { url: "postgres://prod".into() }
///     }
/// }
///
/// let container = Container::new();
/// let shared: Arc<Pool> = container.resolve();
///
/// let plain = container.scope();
/// let from_plain: Arc<Pool> = plain.resolve();
/// assert!(Arc::ptr_eq(&from_plain, &shared));
///
/// let replica = Arc::new(Pool { url: "postgres://replica".into() });
/// let overridden = container.scope().override_singleton(replica.clone());
/// let from_overridden: &Pool = overridden.resolve();
/// assert_eq!(from_overridden.url, "postgres://replica");
///
/// let after: Arc<Pool> = container.resolve();
/// assert!(Arc::ptr_eq(&after, &shared));
/// ```
pub struct Scoped<'p, Parent, TList> {
    parent: &'p Container<Parent>,
    provided: TList,
    overrides: HashMap<TypeKey, Arc<dyn Any + Send + Sync>>,
    singleton_overrides: HashMap<TypeKey, Arc<dyn Any + Send + Sync>>,
}

impl<'p, Parent> Scoped<'p, Parent, ScopeEnd> {
    pub(crate) fn new(parent: &'p Container<Parent>) -> Self {
        Scoped {
            parent,
            provided: ScopeEnd,
            overrides: HashMap::new(),
            singleton_overrides: HashMap::new(),
        }
    }
}

impl<'p, Parent, TList> Scoped<'p, Parent, TList> {
    /// Adds `T` to the scope's type.
    ///
    /// `T` must not also be provided by the parent or have a
    /// [`FromContainer`] impl as shadowing Parent's values is not supported.
    ///
    /// Use [`override_with`](Self::override_with) to override a
    /// type that has a [`FromContainer`] impl.
    pub fn provide<T>(self, value: T) -> Scoped<'p, Parent, Cons<T, TList>> {
        Scoped {
            parent: self.parent,
            provided: Cons {
                head: value,
                tail: self.provided,
            },
            overrides: self.overrides,
            singleton_overrides: self.singleton_overrides,
        }
    }

    /// Override a type that has a [`FromContainer`] impl, for values this scope builds.
    ///
    /// Takes precedence over the parent's override. Singletons are unaffected,
    /// use [`override_singleton`](Self::override_singleton) to override them.
    pub fn override_with<T, I>(mut self, value: T) -> Self
    where
        T: for<'c> FromContainer<I, &'c Self> + Clone + Send + Sync,
    {
        // A factory rather than the bare value, so `AutoBuild` doesn't require `T: Clone`.
        let factory: Box<dyn Fn() -> T + Send + Sync> = Box::new(move || value.clone());
        self.overrides.insert(TypeKey::of::<T>(), Arc::new(factory));
        self
    }

    /// Sets the instance behind `Arc<T>` and `&T` in this scope only.
    ///
    /// The parent and other scopes keep the parent's singleton.
    /// Singletons the parent builds keep using the parent's instance for their own dependencies.
    pub fn override_singleton<T, I>(mut self, value: Arc<T>) -> Self
    where
        T: ?Sized + Shared<I, &'p Container<Parent>, Stored = T> + Send + Sync + 'static,
    {
        self.singleton_overrides
            .insert(TypeKey::of::<T>(), Arc::new(value));
        self
    }

    /// Resolves a single dependency, which may borrow from the scope or the parent.
    ///
    /// See [`Resolve`] for details.
    pub fn resolve<'c, T, I>(&'c self) -> T
    where
        &'c Self: Resolve<I, T>,
    {
        <&'c Self as Resolve<I, T>>::resolve(&self)
    }

    /// Resolves a tuple of dependencies.
    ///
    /// See [`ResolveAll`] for details.
    pub fn resolve_all<'c, T, I>(&'c self) -> T
    where
        T: crate::sealed::Tuple,
        &'c Self: ResolveAll<I, T>,
    {
        <&'c Self as ResolveAll<I, T>>::resolve_all(&self)
    }

    fn build<'c, T, I>(&'c self) -> T
    where
        T: FromContainer<I, &'c Self>,
    {
        let key = TypeKey::of::<T>();
        let found = self
            .overrides
            .get(&key)
            .or_else(|| self.parent.override_for(key));
        if let Some(f) = found {
            // Keyed by TypeId::of::<T>() in override_with, so this can't fail.
            let f = f
                .downcast_ref::<Box<dyn Fn() -> T + Send + Sync>>()
                .unwrap();
            return f();
        }
        T::from_container(self)
    }

    fn singleton<'c, T, I>(&'c self) -> &'c Arc<T>
    where
        T: ?Sized + Shared<I, &'p Container<Parent>> + 'p,
    {
        if let Some(s) = self.singleton_overrides.get(&TypeKey::of::<T::Stored>()) {
            // Keyed by TypeId::of::<T>() in override_singleton, so this can't fail.
            return T::from_stored(s.downcast_ref().unwrap());
        }
        self.parent.singleton::<T, I>()
    }
}

/// Lists the parent, the provided values and the types of the overrides and singleton overrides.
impl<Parent: fmt::Debug, TList: fmt::Debug> fmt::Debug for Scoped<'_, Parent, TList> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scoped")
            .field("parent", self.parent)
            .field("provided", &self.provided)
            .field(
                "overrides",
                &sorted_by_name(self.overrides.keys().copied().collect()),
            )
            .field(
                "singleton_overrides",
                &sorted_by_name(self.singleton_overrides.keys().copied().collect()),
            )
            .finish()
    }
}

impl<'c, 'p, Parent, TList, T, I> Resolve<I, T> for &'c Scoped<'p, Parent, TList>
where
    TList: Locate<'c, T, I, &'c Scoped<'p, Parent, TList>>,
    TList::Out: IntoValue<T>,
{
    fn resolve(&self) -> T {
        let c: &'c Scoped<'p, Parent, TList> = self;
        c.provided.locate(c).into_value()
    }
}

// End of the scope's list: continue in the parent's.
impl<'c, 'p, T, I, Parent, TList> Locate<'c, T, InParent<I>, &'c Scoped<'p, Parent, TList>>
    for ScopeEnd
where
    Parent: Locate<'c, T, I, &'c Scoped<'p, Parent, TList>>,
{
    type Out = Parent::Out;

    fn locate(&'c self, c: &'c Scoped<'p, Parent, TList>) -> Self::Out {
        c.parent.provided().locate(c)
    }
}

// End of the parent's list, reached from a scope: build in the scope, share from the parent.
impl<'c, 'p, T, I, Parent, TList> Locate<'c, T, AutoBuild<I>, &'c Scoped<'p, Parent, TList>> for Nil
where
    T: FromContainer<I, &'c Scoped<'p, Parent, TList>>,
{
    type Out = T;

    fn locate(&'c self, c: &'c Scoped<'p, Parent, TList>) -> T {
        c.build::<T, I>()
    }
}

impl<'c, 'p, T, I, Parent, TList> Locate<'c, Arc<T>, SharedArc<I>, &'c Scoped<'p, Parent, TList>>
    for Nil
where
    T: ?Sized + Shared<I, &'p Container<Parent>> + 'p,
{
    type Out = Arc<T>;

    fn locate(&'c self, c: &'c Scoped<'p, Parent, TList>) -> Arc<T> {
        c.singleton::<T, I>().clone()
    }
}

impl<'c, 'p, T, I, Parent, TList> Locate<'c, &'c T, SharedRef<I>, &'c Scoped<'p, Parent, TList>>
    for Nil
where
    T: ?Sized + Shared<I, &'p Container<Parent>> + 'p,
{
    type Out = &'c T;

    fn locate(&'c self, c: &'c Scoped<'p, Parent, TList>) -> &'c T {
        c.singleton::<T, I>()
    }
}
