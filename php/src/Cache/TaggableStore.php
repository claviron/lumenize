<?php

declare(strict_types=1);

namespace Lumenize\Cache;

use Illuminate\Cache\TaggedCache;
use InvalidArgumentException;
use TypeError;
use ValueError;

/**
 * A `Store` whose runtime store supports tags: `Store::open()` returns it
 * only when `claviron_cache_supports_tags()` says so for the resolved store.
 * Laravel's `Repository::supportsTags()` looks for this `tags()` method.
 *
 * `tags()` returns Laravel's own `TaggedCache`. Its keys are namespaced by
 * the ordered tag names, as in Laravel, so an item is read back through the
 * same tag list it was stored with. Its writes are tagged in the runtime
 * store and its `flush()` invalidates those tags there, so flushing one tag
 * also removes what was stored under a longer list containing it. Runtime
 * tags carry this store's prefix, as its keys do.
 */
final class TaggableStore extends Store
{
    /**
     * @param  string|list<string>  $names
     */
    public function tags($names): TaggedCache
    {
        $names = is_array($names) ? array_values($names) : func_get_args();

        if ($names === []) {
            throw new InvalidArgumentException('A Claviron tagged cache needs at least one tag.');
        }

        foreach ($names as $position => $name) {
            if (!is_string($name)) {
                throw new TypeError(sprintf(
                    'Claviron cache tags must be strings, %s given at position %d.',
                    get_debug_type($name),
                    $position,
                ));
            }

            if (preg_match('//u', $name) !== 1) {
                throw new ValueError("Claviron cache tags must be valid UTF-8, invalid at position {$position}.");
            }
        }

        $scope = new TagScope($this, $names);

        return new TaggedCache($scope, new TagSet($scope, $names));
    }

    /**
     * The runtime tags for Laravel tag names: this store's prefix scopes
     * tags as it scopes keys.
     *
     * @param  list<string>  $names
     * @return list<string>
     */
    public function runtimeTags(array $names): array
    {
        return array_map(fn (string $name): string => $this->prefix.$name, $names);
    }

    /**
     * @internal  The write path of a tagged cache.
     *
     * @param  list<string>  $tags  runtime tags, from `runtimeTags()`
     */
    public function putTagged(string $key, mixed $value, array $tags, ?int $seconds): bool
    {
        return claviron_cache_put_tagged($this->cache, $this->prefix.$key, self::encode($value), $tags, $seconds);
    }

    /**
     * @internal  A tagged cache's flush.
     *
     * @param  list<string>  $tags  runtime tags, from `runtimeTags()`
     */
    public function invalidateTags(array $tags): bool
    {
        return claviron_cache_invalidate_tags($this->cache, $tags);
    }
}
