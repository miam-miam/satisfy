use satisfy::{Container, FromContainer, TryFromContainer, TryResolve, TryResolveAll};
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Clone)]
struct DbUrl(&'static str);

#[derive(Clone, Debug, PartialEq)]
struct DbDown;

#[derive(Clone)]
struct Db {
    url: &'static str,
}

impl<I, C> TryFromContainer<I, C> for Db
where
    // Should be equivalent to Resolve<I, DbUrl>
    C: TryResolve<I, DbUrl, Error = Infallible>,
{
    type Error = DbDown;

    fn try_from_container(c: C) -> Result<Self, DbDown> {
        let Ok(url) = c.try_resolve();
        if url.0.is_empty() {
            Err(DbDown)
        } else {
            Ok(Db { url: url.0 })
        }
    }
}

struct Clock;

impl<C> FromContainer<(), C> for Clock {
    fn from_container(_: C) -> Self {
        Clock
    }
}

#[derive(Debug, PartialEq)]
enum AppError {
    Db(DbDown),
}

impl From<DbDown> for AppError {
    fn from(e: DbDown) -> Self {
        AppError::Db(e)
    }
}

struct Orders;

impl<I, C> TryFromContainer<I, C> for Orders
where
    C: TryResolveAll<I, (Arc<Db>, Clock), Error = DbDown>,
{
    type Error = AppError;

    fn try_from_container(c: C) -> Result<Self, AppError> {
        let (_db, _clock) = c.try_resolve_all()?;
        Ok(Orders)
    }
}

#[test]
fn dependency_error_propagates_to_the_dependent() {
    let c = Container::new().provide(DbUrl(""));
    assert_eq!(
        c.try_resolve::<Orders, _>().err(),
        Some(AppError::Db(DbDown))
    );
}

#[test]
fn plain_provided_and_borrowed_types_mix_with_a_fallible_one() {
    let c = Container::new().provide(DbUrl("pg://mix"));

    let _: (Db, Clock, DbUrl, &DbUrl) = c.try_resolve_all().unwrap();
}

struct Builds(AtomicU32);

struct Audit;

impl<'c, I, C> TryFromContainer<I, C> for Audit
where
    C: TryResolve<I, &'c Builds, Error = Infallible>,
{
    type Error = DbDown;

    fn try_from_container(c: C) -> Result<Self, DbDown> {
        let Ok(builds) = c.try_resolve();
        builds.0.fetch_add(1, Ordering::Relaxed);
        Ok(Audit)
    }
}

#[test]
fn try_resolve_all_stops_at_the_first_error() {
    let c = Container::new()
        .provide(DbUrl(""))
        .provide(Builds(AtomicU32::new(0)));

    let result: Result<(Db, Audit), DbDown> = c.try_resolve_all();
    assert_eq!(result.err(), Some(DbDown));

    let builds: &Builds = c.resolve();
    assert_eq!(builds.0.load(Ordering::Relaxed), 0);
}

// Fails on the first build only.
struct Flaky;

impl<'c, I, C> TryFromContainer<I, C> for Flaky
where
    C: TryResolve<I, &'c Builds, Error = Infallible>,
{
    type Error = DbDown;

    fn try_from_container(c: C) -> Result<Self, DbDown> {
        let Ok(builds) = c.try_resolve();
        match builds.0.fetch_add(1, Ordering::Relaxed) {
            0 => Err(DbDown),
            _ => Ok(Flaky),
        }
    }
}

#[test]
fn failed_singleton_is_rebuilt_and_success_is_cached() {
    let c = Container::new().provide(Builds(AtomicU32::new(0)));

    assert!(c.try_resolve::<Arc<Flaky>, _>().is_err());
    let first: Arc<Flaky> = c.try_resolve().unwrap();
    let again: &Flaky = c.try_resolve().unwrap();

    assert!(std::ptr::eq(&*first, again));
    let builds: &Builds = c.resolve();
    assert_eq!(builds.0.load(Ordering::Relaxed), 2);
}

trait Notifier: Send + Sync {}

impl Notifier for Flaky {}

satisfy::try_dyn_default!(dyn Notifier => Flaky);

#[test]
fn try_dyn_default_rebuilds_after_failure_and_shares_the_impls_singleton() {
    let c = Container::new().provide(Builds(AtomicU32::new(0)));

    assert!(c.try_resolve::<Arc<dyn Notifier>, _>().is_err());
    let notifier: Arc<dyn Notifier> = c.try_resolve().unwrap();
    let flaky: Arc<Flaky> = c.try_resolve().unwrap();
    let borrowed: &dyn Notifier = c.try_resolve().unwrap();

    assert!(std::ptr::addr_eq(&*notifier, &*flaky));
    assert!(std::ptr::addr_eq(&*notifier, borrowed));
}

fn db_and_clock<C, J>(c: &C) -> Result<&'static str, DbDown>
where
    C: TryResolveAll<J, (Clock, Db), Error = DbDown>,
{
    let (_clock, db) = c.try_resolve_all()?;
    Ok(db.url)
}

fn hand_off_subset<C, I>(c: &C) -> Result<&'static str, DbDown>
where
    C: TryResolveAll<I, (Db, DbUrl, Clock), Error = DbDown>,
{
    db_and_clock(c)
}

#[test]
fn try_resolve_all_bound_implies_a_reordered_subset() {
    let c = Container::new().provide(DbUrl("pg://subset"));

    // `&&c` makes `C = &Container`, keeping the by-value impls out of this test.
    assert_eq!(hand_off_subset(&&c), Ok("pg://subset"));
}

fn db_url_via<C, I>(c: Arc<C>) -> Result<&'static str, DbDown>
where
    C: TryResolve<I, Db, Error = DbDown>,
{
    Ok(c.try_resolve()?.url)
}

#[test]
fn container_held_by_value_try_resolves_owned_values() {
    let c = Arc::new(Container::new().provide(DbUrl("pg://owned")));
    assert_eq!(db_and_clock(&*c), Ok("pg://owned"));
    assert_eq!(db_url_via(c), Ok("pg://owned"));
}

#[test]
fn scope_singleton_override_beats_a_failing_parent_default() {
    let app = Container::new().provide(DbUrl(""));
    assert!(app.try_resolve::<Arc<Db>, _>().is_err());

    let replica = Arc::new(Db {
        url: "pg://replica",
    });
    let scope = app.scope().override_singleton(replica.clone());

    let db: Arc<Db> = scope.try_resolve().unwrap();
    assert!(Arc::ptr_eq(&db, &replica));
}

#[test]
fn override_with_replaces_a_failing_default() {
    let c = Container::new()
        .provide(DbUrl(""))
        .override_with(Db { url: "pg://fake" });

    let db: Db = c.try_resolve().unwrap();
    let shared: Arc<Db> = c.try_resolve().unwrap();
    assert_eq!((db.url, shared.url), ("pg://fake", "pg://fake"));
}

#[test]
fn scope_and_parent_overrides_replace_a_failing_default() {
    let app = Container::new()
        .provide(DbUrl(""))
        .override_with(Db { url: "pg://parent" });
    let from_parent: Db = app.scope().try_resolve().unwrap();
    assert_eq!(from_parent.url, "pg://parent");

    let scope = app.scope().override_with(Db { url: "pg://scope" });
    let from_scope: Db = scope.try_resolve().unwrap();
    assert_eq!(from_scope.url, "pg://scope");
}

#[test]
fn scope_borrows_the_parents_fallible_dyn_singleton() {
    let app = Container::new().provide(Builds(AtomicU32::new(0)));
    let scope = app.scope();

    assert!(scope.try_resolve::<&dyn Notifier, _>().is_err());
    let notifier: &dyn Notifier = scope.try_resolve().unwrap();
    let flaky: Arc<Flaky> = app.try_resolve().unwrap();
    assert!(std::ptr::addr_eq(notifier, &*flaky));
}

#[test]
fn override_singleton_replaces_a_failing_default() {
    let fake = Arc::new(Db { url: "pg://fake" });
    let c = Container::new()
        .provide(DbUrl(""))
        .override_singleton(fake.clone());

    let db: Arc<Db> = c.try_resolve().unwrap();
    assert!(Arc::ptr_eq(&db, &fake));
}

fn db_url_from<C, I>(c: &C) -> Result<&'static str, DbDown>
where
    C: TryResolve<I, Db, Error = DbDown>,
{
    Ok(c.try_resolve()?.url)
}

#[test]
fn scope_by_value_try_resolves_owned_values() {
    let app = Container::new();
    let scope = app.scope().provide(DbUrl("pg://scoped"));

    assert_eq!(db_url_from(&scope), Ok("pg://scoped"));
}

fn try_clock<C, J>(c: &C) -> Result<Clock, DbDown>
where
    C: TryResolve<J, Clock, Error = Infallible>,
{
    let Ok(clock) = c.try_resolve();
    Ok(clock)
}

fn plain_hand_off<C, I>(c: &C) -> Result<Clock, DbDown>
where
    C: satisfy::Resolve<I, Clock>,
{
    try_clock(c)
}

#[test]
fn resolve_bound_implies_try_resolve() {
    let c = Container::new();
    assert!(plain_hand_off(&&c).is_ok());
}

fn clock_and_url<C, J>(c: &C) -> Result<&'static str, DbDown>
where
    C: TryResolveAll<J, (DbUrl, Clock), Error = DbDown>,
{
    let (url, _clock) = c.try_resolve_all()?;
    Ok(url.0)
}

fn plain_tuple_hand_off<C, I>(c: &C) -> Result<&'static str, DbDown>
where
    C: satisfy::ResolveAll<I, (Clock, DbUrl)>,
{
    clock_and_url(c)
}

#[test]
fn resolve_all_bound_implies_try_resolve_all_with_any_error() {
    let c = Container::new().provide(DbUrl("pg://plain"));
    assert_eq!(plain_tuple_hand_off(&&c), Ok("pg://plain"));
}

#[derive(Clone)]
struct Tenant(&'static str);

fn tenant_and_db<C, I>(c: &C) -> Result<&'static str, DbDown>
where
    C: TryResolveAll<I, (Tenant, Arc<Db>), Error = DbDown>,
{
    let (tenant, _db) = c.try_resolve_all()?;
    Ok(tenant.0)
}

#[test]
fn scope_mixes_its_own_values_with_parent_fallible_singletons() {
    let app = Container::new().provide(DbUrl("pg://parent"));
    let scope = app.scope().provide(Tenant("acme"));

    let (tenant, db, borrowed): (Tenant, Arc<Db>, &Db) = scope.try_resolve_all().unwrap();
    let parent_db: Arc<Db> = app.try_resolve().unwrap();
    assert_eq!(tenant.0, "acme");
    assert!(Arc::ptr_eq(&db, &parent_db));
    assert!(std::ptr::eq(borrowed, &*parent_db));

    assert_eq!(tenant_and_db(&scope), Ok("acme"));
}

#[test]
fn error_override_fails_a_working_default_and_its_dependents() {
    let c = Container::new()
        .provide(DbUrl("pg://up"))
        .override_with_error::<Db, _, _>(DbDown);

    assert_eq!(c.try_resolve::<Db, _>().err(), Some(DbDown));
    assert_eq!(c.try_resolve::<&Db, _>().err(), Some(DbDown));
    assert_eq!(
        c.try_resolve::<Orders, _>().err(),
        Some(AppError::Db(DbDown))
    );
}

#[test]
fn last_override_of_a_type_wins() {
    let failing = Container::new()
        .provide(DbUrl("pg://up"))
        .override_with(Db { url: "pg://fake" })
        .override_with_error::<Db, _, _>(DbDown);
    assert_eq!(failing.try_resolve::<Db, _>().err(), Some(DbDown));

    let working = Container::new()
        .provide(DbUrl(""))
        .override_with_error::<Db, _, _>(DbDown)
        .override_with(Db { url: "pg://fake" });
    assert_eq!(working.try_resolve::<Db, _>().unwrap().url, "pg://fake");
}

#[test]
fn scope_error_override_beats_the_parents_value_but_not_its_singleton() {
    let app = Container::new()
        .provide(DbUrl(""))
        .override_with(Db { url: "pg://parent" });
    let scope = app.scope().override_with_error::<Db, _, _>(DbDown);

    assert_eq!(scope.try_resolve::<Db, _>().err(), Some(DbDown));
    let shared: Arc<Db> = scope.try_resolve().unwrap();
    assert_eq!(shared.url, "pg://parent");
}

#[test]
fn singleton_override_beats_an_error_override() {
    let fake = Arc::new(Db { url: "pg://fake" });
    let c = Container::new()
        .provide(DbUrl("pg://up"))
        .override_with_error::<Db, _, _>(DbDown)
        .override_singleton(fake.clone());

    let db: &Db = c.try_resolve().unwrap();
    assert!(std::ptr::eq(db, &*fake));
    assert_eq!(c.try_resolve::<Db, _>().err(), Some(DbDown));
}

#[test]
fn scope_value_override_beats_the_parents_error() {
    let app = Container::new()
        .provide(DbUrl("pg://up"))
        .override_with_error::<Db, _, _>(DbDown);
    let scope = app.scope().override_with(Db { url: "pg://scope" });

    assert_eq!(scope.try_resolve::<Db, _>().unwrap().url, "pg://scope");
    assert_eq!(app.scope().try_resolve::<Db, _>().err(), Some(DbDown));
}
