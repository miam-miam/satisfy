//! Singletons behind `Arc<T>`/`&T` and the dyn macros used to
//! create singletons for `dyn Trait`.

use std::sync::Arc;

use crate::strategy::{Fallible, Plain};
use crate::{FromContainer, Resolve, TryFromContainer, TryResolve};

/// Builds the singleton behind `Arc<Self>` and `&Self`.
///
/// Every type with a [`FromContainer`] impl has this too. Its singleton is built the same way as a
/// plain `Self`, so [`override_with`](crate::Container::override_with) applies to it as well.
/// A `dyn Trait` can't be returned by value, so it has no `FromContainer` impl.
/// Use [`dyn_default!`](crate::dyn_default) instead.
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

/// Builds the singleton behind `Arc<Self>` and `&Self` for a type whose default can fail.
///
/// A failed build isn't stored, so the next resolve builds again.
#[doc(hidden)]
pub trait TryShared<I, C> {
    /// `Self` with its lifetimes widened to `'static`, which the singleton is stored and keyed as.
    type Stored: ?Sized + Send + Sync + 'static;
    type Error;

    fn try_build_shared(c: C) -> Result<Arc<Self::Stored>, Self::Error>;

    // Shorten the lifetime to match the container.
    fn from_stored(stored: &Arc<Self::Stored>) -> &Arc<Self>;
}

#[diagnostic::do_not_recommend]
impl<T, I, J, C> TryShared<(I, J), C> for T
where
    T: TryFromContainer<J, C> + Send + Sync,
    C: TryResolve<I, T>,
{
    type Stored = T;
    type Error = <C as TryResolve<I, T>>::Error;

    fn try_build_shared(c: C) -> Result<Arc<T>, Self::Error> {
        c.try_resolve().map(Arc::new)
    }

    fn from_stored(stored: &Arc<T>) -> &Arc<T> {
        stored
    }
}

/// Overrides that replace a singleton: the type has a [`Shared`] or [`TryShared`] impl.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no shared instance this container can build",
    label = "no shared instance for `{Self}` to replace",
    note = "give `{Self}` a `FromContainer` or `TryFromContainer` impl; for a `dyn Trait`, use `satisfy::dyn_default!` or `satisfy::try_dyn_default!`",
    note = "if `{Self}` has one, something it depends on can't be resolved here, or it isn't `Send + Sync`; resolve `{Self}` directly to find out"
)]
pub trait HasShared<I, C> {}

impl<T: ?Sized + Shared<I, C, Stored = T>, I, C> HasShared<Plain<I>, C> for T {}

impl<T: ?Sized + TryShared<I, C, Stored = T>, I, C> HasShared<Fallible<I>, C> for T {}

/// Makes `Arc<dyn Trait>` and `&dyn Trait` resolve to the singleton of `Impl`, built from its
/// [`FromContainer`] impl.
///
/// ```
/// # use satisfy::{Container, FromContainer};
/// # use std::sync::Arc;
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
///
/// assert!(std::ptr::addr_eq(&*db, borrowed));
/// assert!(std::ptr::addr_eq(&*db, &*pg));
/// ```
///
/// To swap in another implementation, override the singleton with
/// [`override_singleton`](crate::Container::override_singleton). An `Arc<dyn Trait>` passed to
/// [`override_with`](crate::Container::override_with) instead competes with the default, so resolving
/// `Arc<dyn Trait>` becomes ambiguous.
///
/// ```
/// # use satisfy::{Container, FromContainer};
/// # use std::sync::Arc;
/// trait Dice: Send + Sync {
///     fn roll(&self) -> u8;
/// }
///
/// struct FairDice;
/// impl Dice for FairDice {
///     fn roll(&self) -> u8 {
///         4 // chosen by fair dice roll
///     }
/// }
///
/// impl<C> FromContainer<(), C> for FairDice {
///     fn from_container(_: C) -> Self {
///         FairDice
///     }
/// }
///
/// satisfy::dyn_default!(dyn Dice => FairDice);
///
/// struct LoadedDice(u8);
/// impl Dice for LoadedDice {
///     fn roll(&self) -> u8 {
///         self.0
///     }
/// }
///
/// let loaded: Arc<dyn Dice> = Arc::new(LoadedDice(6));
/// let rigged = Container::new().override_singleton(loaded);
/// let dice: &dyn Dice = rigged.resolve();
///
/// assert_eq!(dice.roll(), 6);
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

/// Like [`dyn_default!`](crate::dyn_default), for an `Impl` whose default can fail: `Arc<dyn Trait>` and
/// `&dyn Trait` [`try_resolve`](crate::Container::try_resolve) to the singleton of `Impl`, built from
/// its [`TryFromContainer`] impl, with that impl's `Error`.
///
/// A failed build isn't stored, so the next `try_resolve` builds again. For an `Impl` whose default
/// can't fail, use [`dyn_default!`](crate::dyn_default).
///
/// ```
/// # use satisfy::{Container, Resolve, TryFromContainer};
/// # use std::sync::Arc;
/// trait Geocoder: Send + Sync {
///     fn locate(&self, place: &str) -> (f32, f32);
/// }
///
/// #[derive(Clone)]
/// struct TileDir(&'static str);
///
/// #[derive(Debug)]
/// struct MissingTiles;
///
/// struct OfflineGeocoder;
///
/// impl Geocoder for OfflineGeocoder {
///     fn locate(&self, _: &str) -> (f32, f32) {
///         (51.5, -0.1)
///     }
/// }
///
/// impl<I, C> TryFromContainer<I, C> for OfflineGeocoder
/// where
///     C: Resolve<I, TileDir>,
/// {
///     type Error = MissingTiles;
///
///     fn try_from_container(c: C) -> Result<Self, MissingTiles> {
///         let dir: TileDir = c.resolve();
///         if dir.0.is_empty() { Err(MissingTiles) } else { Ok(OfflineGeocoder) }
///     }
/// }
///
/// satisfy::try_dyn_default!(dyn Geocoder => OfflineGeocoder);
///
/// let c = Container::new().provide(TileDir("/srv/tiles"));
/// let geo: Arc<dyn Geocoder> = c.try_resolve().unwrap();
/// let concrete: Arc<OfflineGeocoder> = c.try_resolve().unwrap();
///
/// assert_eq!(geo.locate("London"), (51.5, -0.1));
/// assert!(std::ptr::addr_eq(&*geo, &*concrete));
/// ```
#[macro_export]
macro_rules! try_dyn_default {
    (dyn $trait:path => $impl:ty) => {
        // For every lifetime, so a `&'c dyn Trait` bound (which means `dyn Trait + 'c`) resolves.
        impl<'a, I, C> $crate::TryShared<I, C> for dyn $trait + 'a
        where
            // Only through `Fallible`, so a plain `Impl` is rejected rather than given a singleton that
            // `try_resolve` reaches and `resolve` doesn't.
            C: $crate::TryResolve<$crate::strategy::Fallible<I>, ::std::sync::Arc<$impl>>,
        {
            type Stored = dyn $trait;
            type Error =
                <C as $crate::TryResolve<$crate::strategy::Fallible<I>, ::std::sync::Arc<$impl>>>::Error;

            fn try_build_shared(
                c: C,
            ) -> ::std::result::Result<::std::sync::Arc<dyn $trait>, Self::Error> {
                <C as $crate::TryResolve<$crate::strategy::Fallible<I>, ::std::sync::Arc<$impl>>>::try_resolve(&c)
                    .map(|built| built as ::std::sync::Arc<dyn $trait>)
            }

            fn from_stored(stored: &::std::sync::Arc<dyn $trait>) -> &::std::sync::Arc<Self> {
                stored
            }
        }
    };
}
