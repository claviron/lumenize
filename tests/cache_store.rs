//! The `claviron` Laravel cache driver through Laravel's `CacheManager` and
//! the real PHP cache functions: which runtime store a Laravel store
//! selects, and whether it offers tags.
//!
//! The runtime's catalog: `fleet`, the default, is an Orbit store without a
//! backing store, so it supports tags; `local` and `redis` are memory
//! stores, which do not. The Orbit store uses an in-memory fleet, so the test
//! maps no shared memory.
//!
//! The fixture needs a Laravel application's Composer autoloader with this
//! package installed: `LUMENIZE_COMPOSER_AUTOLOAD`, or the example
//! application's after `composer install` in `examples/laravel`.

#[path = "support/native_runtime.rs"]
mod native_runtime;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use claviron::cache::config::CacheStoreConfig;
use claviron::cache::{
    CacheConfig, CacheProvider, CacheRegistry, OrbitCacheStore,
};
use claviron::orbit::Fleet;
use claviron::orbit::cache::CacheService;

#[tokio::test]
async fn laravel_stores_select_runtime_stores_and_their_tag_support() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let autoload = std::env::var_os("LUMENIZE_COMPOSER_AUTOLOAD")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("examples/laravel/vendor/autoload.php"));
    assert!(
        autoload.is_file(),
        "{} is missing: run `composer install` in examples/laravel, or set LUMENIZE_COMPOSER_AUTOLOAD",
        autoload.display()
    );
    let entry = tempfile::tempdir().unwrap();
    let script = entry.path().join("cache_store.php");
    std::fs::write(
        &script,
        format!(
            "<?php require {}; require {};\n",
            native_runtime::php_string(&autoload),
            native_runtime::php_string(
                &root.join("tests/fixtures/cache_store.php")
            ),
        ),
    )
    .unwrap();

    native_runtime::run_fixture(&script, async |state| {
        let memory = CacheStoreConfig::default;
        claviron::cache::register_with_config(
            state,
            CacheConfig {
                default: "fleet".to_owned(),
                stores: HashMap::from([
                    ("local".to_owned(), memory()),
                    ("redis".to_owned(), memory()),
                ]),
            },
        )
        .unwrap();
        let fleet = Fleet::join("lumenize-cache-store-test", 1).unwrap();
        let service = CacheService::from_fleet(Arc::new(fleet)).unwrap();
        let registry = state.registry_ref();
        registry
            .resolve::<CacheRegistry>()
            .unwrap()
            .register_store(Arc::new(
                OrbitCacheStore::new("fleet", service, 1024, None).unwrap(),
            ))
            .unwrap();
        let provider = registry.resolve::<CacheProvider>().unwrap();
        claviron::runtime::AnyComponent::boot(
            provider.as_ref(),
            registry,
            state,
        )
        .await
        .unwrap();
    })
    .await;
}
