use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::container::{
    insert_error_override, insert_value_override, run_override, run_value_override,
};
use crate::once_map::{TypeKey, sorted_by_name};
use crate::strategy::{
    AutoBuild, Fallible, InParent, IntoValue, Locate, SameError, SharedArc, SharedRef,
    TryAutoBuild, TrySharedArc, TrySharedRef, TryWalk,
};
use crate::typed_list::{Cons, Nil, ScopeEnd};
use crate::{
    Container, FromContainer, HasDefault, HasShared, Resolve, ResolveAll, Shared, TryFromContainer,
    TryResolve, TryResolveAll, TryResolveIn, TryShared,
};

/// A child of a [`Container`], made with [`Container::scope`], allowing request-based overrides.
///
/// It resolves its own provided values and overrides first,
/// before checking the parent's values.
///
/// It uses the parent's singletons, unless the scope sets one with
/// [`override_singleton`](Self::override_singleton) which overrides the singleton used for
/// that scope only.
///
/// ```
/// # use satisfy::{Container, FromContainer};
/// # use std::sync::Arc;
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
///
/// assert!(Arc::ptr_eq(&from_plain, &shared));
///
/// let replica = Arc::new(Pool { url: "postgres://replica".into() });
/// let overridden = container.scope().override_singleton(replica.clone());
/// let from_overridden: &Pool = overridden.resolve();
///
/// assert_eq!(from_overridden.url, "postgres://replica");
///
/// let after: Arc<Pool> = container.resolve();
///
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
    /// [`FromContainer`] impl as shadowing parent values is not supported.
    ///
    /// Use [`override_with`](Self::override_with) to override a
    /// type that has a [`FromContainer`] impl.
    #[must_use]
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
    #[must_use]
    pub fn override_with<T, I>(mut self, value: T) -> Self
    where
        T: for<'c> HasDefault<I, &'c Self> + Clone + Send + Sync + 'static,
    {
        insert_value_override(&mut self.overrides, value);
        self
    }

    /// Makes [`try_resolve`](Self::try_resolve) of a type with a [`TryFromContainer`] impl fail
    /// with a clone of the provided `error`.
    ///
    /// Like [`override_with`](Self::override_with), it takes precedence over the parent's
    /// override and leaves singletons unaffected.
    #[must_use]
    pub fn override_with_error<T, E, I>(mut self, error: E) -> Self
    where
        T: for<'c> TryFromContainer<I, &'c Self, Error = E>,
        E: Clone + Send + Sync + 'static,
    {
        insert_error_override::<T, E>(&mut self.overrides, error);
        self
    }

    /// Sets the instance behind `Arc<T>` and `&T` in this scope only.
    ///
    /// The parent and other scopes keep the parent's singleton.
    /// Singletons the parent builds keep using the parent's instance for their own dependencies.
    #[must_use]
    pub fn override_singleton<T, I>(mut self, value: Arc<T>) -> Self
    where
        T: ?Sized + HasShared<I, &'p Container<Parent>> + Send + Sync + 'static,
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
        T: crate::resolve_all::sealed::Tuple,
        &'c Self: ResolveAll<I, T>,
    {
        <&'c Self as ResolveAll<I, T>>::resolve_all(&self)
    }

    /// Resolves a single dependency whose default may fail, such as one built from a
    /// [`TryFromContainer`] impl.
    ///
    /// See [`TryResolve`] for details.
    pub fn try_resolve<'c, T, I>(&'c self) -> Result<T, <&'c Self as TryResolve<I, T>>::Error>
    where
        &'c Self: TryResolve<I, T>,
    {
        <&'c Self as TryResolve<I, T>>::try_resolve(&self)
    }

    /// Resolves a tuple of dependencies, whose fallible elements share one error type.
    ///
    /// See [`TryResolveAll`] for details.
    pub fn try_resolve_all<'c, T, I>(
        &'c self,
    ) -> Result<T, <&'c Self as TryResolveAll<I, T>>::Error>
    where
        T: crate::resolve_all::sealed::Tuple,
        &'c Self: TryResolveAll<I, T>,
    {
        <&'c Self as TryResolveAll<I, T>>::try_resolve_all(&self)
    }

    // The scope's override wins over the parent's.
    fn overridden<T: 'static>(&self) -> Option<T> {
        match self.overrides.get(&TypeKey::of::<T>()) {
            Some(f) => run_value_override(f),
            None => self.parent.overridden(),
        }
    }

    fn try_overridden<T: 'static, E: 'static>(&self) -> Option<Result<T, E>> {
        match self.overrides.get(&TypeKey::of::<T>()) {
            Some(f) => Some(run_override(f)),
            None => self.parent.try_overridden(),
        }
    }

    fn build<'c, T, I>(&'c self) -> T
    where
        T: FromContainer<I, &'c Self>,
    {
        self.overridden().unwrap_or_else(|| T::from_container(self))
    }

    fn try_build<'c, T, I>(&'c self) -> Result<T, T::Error>
    where
        T: TryFromContainer<I, &'c Self>,
    {
        self.try_overridden()
            .unwrap_or_else(|| T::try_from_container(self))
    }

    fn try_singleton<'c, T, I>(&'c self) -> Result<&'c Arc<T>, T::Error>
    where
        T: ?Sized + TryShared<I, &'p Container<Parent>> + 'p,
    {
        if let Some(s) = self.singleton_overrides.get(&TypeKey::of::<T::Stored>()) {
            // Keyed by TypeId::of::<T>() in override_singleton, so this can't fail.
            return Ok(T::from_stored(s.downcast_ref().unwrap()));
        }
        self.parent.try_singleton::<T, I>()
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

// Plain resolves are covered for every try resolve by the blanket `Plain` impl.
impl<'c, 'p, Parent, TList, T, I> TryResolve<Fallible<I>, T> for &'c Scoped<'p, Parent, TList>
where
    TList: TryWalk<'c, T, I, &'c Scoped<'p, Parent, TList>>,
{
    type Error = TList::Error;

    fn try_resolve(&self) -> Result<T, Self::Error> {
        let c: &'c Scoped<'p, Parent, TList> = self;
        c.provided.try_walk(c)
    }
}

impl<'c, 'p, Parent, TList, T, I, E> TryResolveIn<Fallible<I>, T, E>
    for &'c Scoped<'p, Parent, TList>
where
    TList: TryWalk<'c, T, I, &'c Scoped<'p, Parent, TList>>,
    TList::Error: SameError<T, E>,
{
    fn try_resolve_in(&self) -> Result<T, E> {
        let c: &'c Scoped<'p, Parent, TList> = self;
        c.provided.try_walk(c).map_err(SameError::into_same)
    }
}

/// Resolves values that don't borrow from the scope, so generic code can bound
/// `S: Resolve<I, T>` instead of `for<'c> &'c S: Resolve<I, T>`.
impl<'p, Parent, TList, T, I> Resolve<I, T> for Scoped<'p, Parent, TList>
where
    for<'c> &'c Scoped<'p, Parent, TList>: Resolve<I, T>,
{
    fn resolve(&self) -> T {
        <&Scoped<'p, Parent, TList> as Resolve<I, T>>::resolve(&self)
    }
}

/// Resolves values that don't borrow from the scope, like the by-value [`Resolve`] impl.
#[diagnostic::do_not_recommend]
impl<'p, Parent, TList, T, I, E> TryResolve<Fallible<I>, T> for Scoped<'p, Parent, TList>
where
    for<'c> &'c Scoped<'p, Parent, TList>: TryResolve<Fallible<I>, T, Error = E>,
{
    type Error = E;

    fn try_resolve(&self) -> Result<T, E> {
        <&Scoped<'p, Parent, TList> as TryResolve<Fallible<I>, T>>::try_resolve(&self)
    }
}

#[diagnostic::do_not_recommend]
impl<'p, Parent, TList, T, I, E> TryResolveIn<Fallible<I>, T, E> for Scoped<'p, Parent, TList>
where
    for<'c> &'c Scoped<'p, Parent, TList>: TryResolveIn<Fallible<I>, T, E>,
{
    fn try_resolve_in(&self) -> Result<T, E> {
        <&Scoped<'p, Parent, TList> as TryResolveIn<Fallible<I>, T, E>>::try_resolve_in(&self)
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

// End of the scope's list, for a default that can fail: continue in the parent's.
impl<'c, 'p, T, I, Parent, TList> TryWalk<'c, T, InParent<I>, &'c Scoped<'p, Parent, TList>>
    for ScopeEnd
where
    Parent: TryWalk<'c, T, I, &'c Scoped<'p, Parent, TList>>,
{
    type Error = Parent::Error;

    fn try_walk(&'c self, c: &'c Scoped<'p, Parent, TList>) -> Result<T, Self::Error> {
        c.parent.provided().try_walk(c)
    }
}

impl<'c, 'p, T, I, Parent, TList> TryWalk<'c, T, TryAutoBuild<I>, &'c Scoped<'p, Parent, TList>>
    for Nil
where
    T: TryFromContainer<I, &'c Scoped<'p, Parent, TList>>,
{
    type Error = T::Error;

    fn try_walk(&'c self, c: &'c Scoped<'p, Parent, TList>) -> Result<T, T::Error> {
        c.try_build::<T, I>()
    }
}

impl<'c, 'p, T, I, Parent, TList>
    TryWalk<'c, Arc<T>, TrySharedArc<I>, &'c Scoped<'p, Parent, TList>> for Nil
where
    T: ?Sized + TryShared<I, &'p Container<Parent>> + 'p,
{
    type Error = T::Error;

    fn try_walk(&'c self, c: &'c Scoped<'p, Parent, TList>) -> Result<Arc<T>, T::Error> {
        c.try_singleton::<T, I>().cloned()
    }
}

impl<'c, 'p, T, I, Parent, TList> TryWalk<'c, &'c T, TrySharedRef<I>, &'c Scoped<'p, Parent, TList>>
    for Nil
where
    T: ?Sized + TryShared<I, &'p Container<Parent>> + 'p,
{
    type Error = T::Error;

    fn try_walk(&'c self, c: &'c Scoped<'p, Parent, TList>) -> Result<&'c T, T::Error> {
        c.try_singleton::<T, I>().map(|a| &**a)
    }
}
