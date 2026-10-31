# Understanding compile errors

The container checks the dependency graph at compile time, so mistakes show up as trait errors
([E0277](https://doc.rust-lang.org/error_codes/E0277.html)). Most carry a custom message saying what to do. This guide
covers the ones that don't, or that are unclear.

## `type annotations needed` ([E0283](https://doc.rust-lang.org/error_codes/E0283.html))

Two sources match the type you asked for, and the container won't pick one. The error mentions
`multiple impls satisfying ... Locate` and usually comes in pairs. Ignore the suggestion to write
`resolve::<Port, I>()`: the path markers aren't meant to be written by hand. Remove the duplicate instead.

This can be caused by:

- The type was provided twice. Provide it once.
- The type was provided and also has a `FromContainer` impl. Use `override_with` to replace the default instead of
  `provide`.
- A scope provides a type that its parent provides, or that has a default. A scope can't shadow these. Use
  `override_with` on the scope if the type has a default; otherwise provide it only in the scope, not the parent.

## `this container can't resolve Arc<X>` (or `&X`)

This appears for shared instances (singletons). Check the following in order:

1. `X` has a `FromContainer` impl, or was provided.
2. Every dependency of `X` can be resolved. The error stops at `Arc<X>` instead of naming the missing dependency, so
   add a temporary `let _: X = container.resolve();`: its error names the type that's missing.
3. `X` is `Send + Sync`. Singletons are shared across threads, so a type holding an `Rc` or `Cell`
   can't be one.
4. For `Arc<dyn Trait>`, the trait has a `satisfy::dyn_default!(dyn Trait => Impl)`.
5. No `FromContainer` impl pins the path (see below).

## `can't resolve Port` when `Port` is provided

A `FromContainer` impl has pinned the path in its bound:

```rust,ignore
impl<C> FromContainer<(), C> for Server
where
    C: Resolve<(), Port>,
{ ... }
```

`()` belongs in `FromContainer<(), C>` for a type with no dependencies. It isn't a path a `Resolve`
bound can take, so this bound can never be met. Keep the path generic and pass it through instead:

```rust,ignore
impl<I, C> FromContainer<I, C> for Server
where
    C: Resolve<I, Port>,
{ ... }
```

## The error names a type you didn't resolve

Resolving `App` can fail with "`Port` has no default". The error names the type at the bottom of the chain that is
missing, not the one you asked for. The `required for ...` notes underneath trace the path from `App` down to `Port`.
The trace stops early at a shared `Arc<X>` or `&X`; see [above](#this-container-cant-resolve-arcx-or-x).

## `has no default` from `override_with`

`override_with` only replaces an existing default. To supply a value for a type without one, use
`provide`. To swap the implementation behind `Arc<dyn Trait>`, use
`override_singleton::<dyn Trait, _>(..)`; `override_with` doesn't accept an `Arc<dyn Trait>`.

## Paths in error messages

Errors often show types like `There<AutoBuild<_>>`. These record the path the resolve took: here, skip one provided
value, then build from a default. The `strategy` module documents each step. You should never need to write them
yourself.

## `does not live long enough` ([E0597](https://doc.rust-lang.org/error_codes/E0597.html))

A resolved `&T` borrows the container, and a scope borrows its parent. Neither can outlive what it borrows from. Keep
the container alive for as long as the reference or scope, or resolve an owned value instead: `Arc<T>` for a singleton,
or `T` for a provided value that is `Clone`.

## `implementation of IntoValue is not general enough`

A bound on the container itself only covers values that don't borrow from it. This error appears when generic code uses
one to ask for a borrow such as `&Config`:

```rust,ignore
fn config<'a, C, I>(c: &'a C) -> &'a Config
where
    C: Resolve<I, &'a Config>,
{ ... }

config(&container); // `C` is `Container<..>`
```

Take the reference as `C` instead, so the bound is on `&Container<..>`:

```rust,ignore
fn config<'a, C, I>(c: C) -> &'a Config
where
    C: Resolve<I, &'a Config>,
{ ... }

config(&container); // `C` is `&Container<..>`
```
