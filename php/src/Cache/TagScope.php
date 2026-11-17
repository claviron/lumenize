<?php

declare(strict_types=1);

namespace Lumenize\Cache;

use BadMethodCallException;
use Illuminate\Contracts\Cache\Lock as LockContract;
use Illuminate\Contracts\Cache\LockProvider;
use Illuminate\Contracts\Cache\Store as StoreContract;

/**
 * The store one Laravel `TaggedCache` writes through: a `TaggableStore`
 * bound to one tag list. Every write carries the list's runtime tags; reads
 * and deletes are the store's own. Keys arrive already namespaced by
 * `TaggedCache`.
 *
 * Counters are refused: they live in the Orbit counter ring, which keeps no
 * tags, so a counter would survive a flush of its tags. Locks are the store's
 * own and untagged, as Laravel's tagged caches pass `lock()` to their store.
 */
final class TagScope implements StoreContract, LockProvider
{
    /** @var list<string> */
    private readonly array $tags;

    /**
     * @param  list<string>  $names
     */
    public function __construct(private readonly TaggableStore $store, array $names)
    {
        $this->tags = $store->runtimeTags($names);
    }

    public function get($key)
    {
        return $this->store->get($key);
    }

    public function many(array $keys)
    {
        return $this->store->many($keys);
    }

    public function put($key, $value, $seconds)
    {
        return $this->store->putTagged($key, $value, $this->tags, (int) $seconds);
    }

    public function putMany(array $values, $seconds)
    {
        $stored = true;

        foreach ($values as $key => $value) {
            $stored = $this->put($key, $value, $seconds) && $stored;
        }

        return $stored;
    }

    public function forever($key, $value)
    {
        return $this->store->putTagged($key, $value, $this->tags, null);
    }

    /**
     * Read back and stored again with this list's tags, as the untagged
     * store touches.
     */
    public function touch($key, $seconds)
    {
        $value = $this->get($key);

        if ($value === null) {
            return false;
        }

        return $this->put($key, $value, $seconds);
    }

    public function forget($key)
    {
        return $this->store->forget($key);
    }

    /**
     * Laravel's `Repository::clear()` on a tagged cache flushes its whole
     * store; so does this. `TaggedCache::flush()` invalidates the tags.
     */
    public function flush()
    {
        return $this->store->flush();
    }

    public function increment($key, $value = 1)
    {
        throw self::counters();
    }

    public function decrement($key, $value = 1)
    {
        throw self::counters();
    }

    public function lock($name, $seconds = 0, $owner = null): LockContract
    {
        return $this->store->lock($name, $seconds, $owner);
    }

    public function restoreLock($name, $owner): LockContract
    {
        return $this->store->restoreLock($name, $owner);
    }

    public function getPrefix()
    {
        return $this->store->getPrefix();
    }

    /**
     * Invalidate tags in the runtime store: this list's, or the named ones.
     *
     * @param  list<string>|null  $names
     */
    public function invalidate(?array $names = null): bool
    {
        return $this->store->invalidateTags($names === null ? $this->tags : $this->store->runtimeTags($names));
    }

    private static function counters(): BadMethodCallException
    {
        return new BadMethodCallException(
            'Claviron tagged caches do not support increment or decrement: counters live outside the '
            .'cache store, keep no tags and would survive a flush of these tags. Use an untagged counter.',
        );
    }
}
