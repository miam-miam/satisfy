//! The types that record what a container holds, such as `Container<Cons<Config, Nil>>`.
//!
//! Name them to store a container in a struct field. Where possible prefer adding a bound using
//! [`Resolve`](crate::Resolve) instead of writing the list out.
//!
//! Each [`provide`](crate::Container::provide) wraps the list in another `Cons`, so the most
//! recently provided type comes first:
//!
//! ```
//! # use satisfy::Container;
//! # use satisfy::typed_list::{Cons, Nil};
//! #[derive(Clone)]
//! struct Region(&'static str);
//!
//! #[derive(Clone)]
//! struct MaxUploadMb(u32);
//!
//! struct Uploader {
//!     container: Container<Cons<MaxUploadMb, Cons<Region, Nil>>>,
//! }
//!
//! let uploader = Uploader {
//!     container: Container::new().provide(Region("eu-west")).provide(MaxUploadMb(25)),
//! };
//! let region: Region = uploader.container.resolve();
//!
//! assert_eq!(region.0, "eu-west");
//! ```

use std::fmt;

/// A type-level linked list of the types provided to a container. Nil is the empty list.
pub struct Nil;

/// A type-level linked list of the types provided to a
/// container composed of a set of `Cons` that end in [`Nil`].
pub struct Cons<H, T> {
    pub(crate) head: H,
    pub(crate) tail: T,
}

/// Ends a scope's own list of provided values with the parent's list
/// then being searched for the type.
pub struct ScopeEnd;

impl fmt::Debug for ScopeEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().finish()
    }
}

impl fmt::Debug for Nil {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().finish()
    }
}

// Flattened into one list rather than nested `Cons { head, tail }`.
impl<H: fmt::Debug, T: Entries> fmt::Debug for Cons<H, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        self.entries(&mut list);
        list.finish()
    }
}

trait Entries {
    fn entries(&self, list: &mut fmt::DebugList<'_, '_>);
}

impl Entries for Nil {
    fn entries(&self, _: &mut fmt::DebugList<'_, '_>) {}
}

impl Entries for ScopeEnd {
    fn entries(&self, _: &mut fmt::DebugList<'_, '_>) {}
}

impl<H: fmt::Debug, T: Entries> Entries for Cons<H, T> {
    fn entries(&self, list: &mut fmt::DebugList<'_, '_>) {
        list.entry(&self.head);
        self.tail.entries(list);
    }
}
