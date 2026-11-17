<?php

declare(strict_types=1);

namespace Lumenize\Providers;

use Lumenize\Cache\DetachedStore;
use Lumenize\Cache\Store;
use Illuminate\Cache\CacheManager;
use Illuminate\Cache\Repository;
use Illuminate\Contracts\Foundation\Application;
use Illuminate\Support\ServiceProvider;
use InvalidArgumentException;
use Psr\Log\LoggerInterface;

/**
 * Registers the `claviron` cache driver:
 *
 *     'shared' => ['driver' => 'claviron', 'runtime_store' => 'fleet'],
 *
 * `runtime_store` names a store in the runtime's `cache.yaml`, independently
 * of the Laravel store's own name; omitting it, or null, selects the
 * runtime's default store. Any other value that is not a non-empty string is
 * refused, and a name the runtime does not know fails when the store is
 * resolved. Neither falls back to the runtime default or to `fallback`, which
 * applies only where the runtime is absent.
 *
 * The repository supports tags exactly when the resolved runtime store does.
 */
final class CacheServiceProvider extends ServiceProvider
{
    public function boot(CacheManager $cache): void
    {
        // CacheManager binds custom creators to itself, so `self` inside is the
        // manager. Static closures cannot be rebound (PHP 8.5 warns and PHP 9
        // rejects it).
        $selection = self::runtimeStore(...);
        $cache->extend('claviron', function (Application $app, array $config) use ($cache, $selection): Repository {
            $runtimeStore = $selection($config);
            $prefix = (string) ($config['prefix'] ?? $app['config']['cache.prefix'] ?? '');

            if (function_exists('claviron_cache_store')
                && function_exists('claviron_runtime_configured')
                && claviron_runtime_configured()) {
                return $cache->repository(Store::open($runtimeStore, $prefix), $config);
            }

            $fallback = $config['fallback'] ?? null;
            if (is_string($fallback) && $fallback !== '') {
                if ($fallback === ($config['store'] ?? null)) {
                    throw new InvalidArgumentException(
                        "Claviron cache store [{$fallback}] cannot fall back to itself.",
                    );
                }

                return $cache->store($fallback);
            }

            // A shell or a test process. Said once per process, because the
            // manager keeps the store it built.
            $app->make(LoggerInterface::class)->warning(sprintf(
                'Claviron cache store [%s] is running without the runtime: reads miss and writes are dropped.',
                $runtimeStore ?? 'default',
            ));

            return $cache->repository(new DetachedStore($prefix), $config);
        });
    }

    /**
     * The runtime store a Laravel store selects, null for the runtime default.
     * Checked before the runtime is consulted, so a configuration that names
     * no usable store fails the same way attached or detached.
     */
    private static function runtimeStore(array $config): ?string
    {
        $name = $config['runtime_store'] ?? null;

        if ($name === null) {
            return null;
        }

        if (!is_string($name) || trim($name) === '') {
            throw new InvalidArgumentException(sprintf(
                'Claviron cache store [%s]: runtime_store must name a runtime cache store, or be null for the runtime default; %s given.',
                $config['store'] ?? '',
                is_string($name) ? var_export($name, true) : get_debug_type($name),
            ));
        }

        return $name;
    }
}
