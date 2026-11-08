//! Resolving a tuple of dependencies at once.

use crate::Resolve;
use crate::sealed::Resolver;
use crate::strategy::{Plain, Skip, SkipPath};

// Declares one hidden associated type per tuple position.
macro_rules! type_slots {
    ($($slot:ident),+) => {
        $(
            #[doc(hidden)]
            type $slot;
        )+
    };
}

pub(crate) mod sealed {
    #[diagnostic::on_unimplemented(
        message = "`ResolveAll` takes a tuple of 2 to 12 dependencies, not `{Self}`",
        label = "not a tuple of 2 to 12 dependencies",
        note = "to resolve a single dependency, use `Resolve` (`.resolve()`) instead"
    )]
    pub trait Tuple {
        type_slots!(E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12);
    }
}

/// Resolves a tuple of 2 to 12 dependencies, using [`Resolve`] for each element.
///
/// For one dependency, use [`Resolve`].
///
/// `I` is a tuple of the elements' resolution paths, inferred (see [`strategy`](crate::strategy) for more details).
///
/// A `C: ResolveAll<I, (A, B)>` bound also implies `C: Resolve<_, A>` and `C: Resolve<_, B>`, so
/// generic code can hand the elements to code that needs them one at a time with a single `I`.
/// It also implies `ResolveAll` of any 2 or more of those elements, in any order, so
/// `ResolveAll<I, (A, B, C, D)>` covers a call needing `ResolveAll<_, (B, D)>`.
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
    + Resolve<SkipPath, Skip>
{
    type_slots!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12);

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
    ///
    /// assert_eq!(server.port, 8080);
    /// assert!(Arc::ptr_eq(&server.pool, &pool));
    /// ```
    fn resolve_all(&self) -> T;
}

/// Resolves a tuple of 2 to 12 dependencies, using [`TryResolve`](crate::TryResolve) for each element, and stops at
/// the first error.
///
/// Every element built from [`TryFromContainer`](crate::TryFromContainer) must have the same `Error`, which becomes the
/// tuple's; plain elements fit any error. Resolve elements with different errors one at a time.
/// A tuple with no fallible element can't infer its error, so use [`ResolveAll`] for it.
///
/// In generic code, name the error in the bound: `C: TryResolveAll<I, (A, B), Error = E>`. As with
/// [`ResolveAll`], the bound implies `TryResolveAll` of any 2 or more of its elements with the same
/// error, in any order. A `ResolveAll` bound implies `TryResolveAll` of its elements with any error.
#[diagnostic::on_unimplemented(
    message = "this container can't try to resolve every element of `{T}`",
    label = "an element of `{T}` can't be resolved here",
    note = "every fallible element must have the same `Error`, and each element (or something it depends on) needs `.provide(..)`, or a `TryFromContainer` impl whose dependencies are all resolvable"
)]
pub trait TryResolveAll<I, T: sealed::Tuple>:
    TryResolveIn<Self::P1, T::E1, Self::Error>
    + TryResolveIn<Self::P2, T::E2, Self::Error>
    + TryResolveIn<Self::P3, T::E3, Self::Error>
    + TryResolveIn<Self::P4, T::E4, Self::Error>
    + TryResolveIn<Self::P5, T::E5, Self::Error>
    + TryResolveIn<Self::P6, T::E6, Self::Error>
    + TryResolveIn<Self::P7, T::E7, Self::Error>
    + TryResolveIn<Self::P8, T::E8, Self::Error>
    + TryResolveIn<Self::P9, T::E9, Self::Error>
    + TryResolveIn<Self::P10, T::E10, Self::Error>
    + TryResolveIn<Self::P11, T::E11, Self::Error>
    + TryResolveIn<Self::P12, T::E12, Self::Error>
    + TryResolveIn<Plain<SkipPath>, Skip, Self::Error>
{
    type_slots!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12);

    /// The error shared by the fallible elements.
    type Error;

    /// Resolves every element of `T`, returning the first error.
    ///
    /// ```
    /// # use satisfy::{Container, FromContainer, TryFromContainer, TryResolveAll};
    /// # use std::sync::Arc;
    /// #[derive(Clone)]
    /// struct JournalPath(&'static str);
    ///
    /// #[derive(Debug)]
    /// struct Corrupt;
    ///
    /// struct Journal;
    ///
    /// impl<C> TryFromContainer<(), C> for Journal {
    ///     type Error = Corrupt;
    ///
    ///     fn try_from_container(_: C) -> Result<Self, Corrupt> {
    ///         Ok(Journal)
    ///     }
    /// }
    ///
    /// struct Clock;
    ///
    /// impl<C> FromContainer<(), C> for Clock {
    ///     fn from_container(_: C) -> Self {
    ///         Clock
    ///     }
    /// }
    ///
    /// struct Ledger {
    ///     journal: Arc<Journal>,
    ///     path: &'static str,
    /// }
    ///
    /// impl<I, C> TryFromContainer<I, C> for Ledger
    /// where
    ///     C: TryResolveAll<I, (Arc<Journal>, Clock, JournalPath), Error = Corrupt>,
    /// {
    ///     type Error = Corrupt;
    ///
    ///     fn try_from_container(c: C) -> Result<Self, Corrupt> {
    ///         let (journal, _clock, path) = c.try_resolve_all()?;
    ///         Ok(Ledger { journal, path: path.0 })
    ///     }
    /// }
    ///
    /// let c = Container::new().provide(JournalPath("/var/ledger"));
    /// let (ledger, journal): (Ledger, Arc<Journal>) = c.try_resolve_all().unwrap();
    ///
    /// assert_eq!(ledger.path, "/var/ledger");
    /// assert!(Arc::ptr_eq(&ledger.journal, &journal));
    /// ```
    fn try_resolve_all(&self) -> Result<T, Self::Error>;
}

/// Helper trait that resolves one element of a [`TryResolveAll`] tuple whose shared error is `E`.
///
/// A plain element fits any `E`;
/// one built from [`TryFromContainer`](crate::TryFromContainer) fits only its own `Error`.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "this container can't try to resolve `{T}`",
    label = "`{T}` can't be resolved here",
    note = "`{T}` (or something it depends on) needs `.provide(..)`, or a `TryFromContainer` impl whose dependencies are all resolvable; for an `Arc<dyn Trait>` or `&dyn Trait`, `satisfy::try_dyn_default!`"
)]
pub trait TryResolveIn<I, T, E>: Resolver {
    fn try_resolve_in(&self) -> Result<T, E>;
}

impl<C, I, T, E> TryResolveIn<Plain<I>, T, E> for C
where
    C: Resolve<I, T>,
{
    fn try_resolve_in(&self) -> Result<T, E> {
        Ok(self.resolve())
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

        // The shared error rides in the path, after the elements' paths, so it is inferred.
        impl<C, E, $($t, $i),+> TryResolveAll<($($i,)+ E,), ($($t,)+)> for C
        where
            $(C: TryResolveIn<$i, $t, E>,)+
            C: TryResolveIn<Plain<SkipPath>, Skip, E>,
        {
            $(type $p = $i;)+
            $($(type $pp = Plain<SkipPath>;)+)?
            type Error = E;

            fn try_resolve_all(&self) -> Result<($($t,)+), E> {
                Ok(($(<C as TryResolveIn<$i, $t, E>>::try_resolve_in(self)?,)+))
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
