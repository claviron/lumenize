<?php

declare(strict_types=1);

// Laravel's CacheManager over the real cache functions. The runtime has
// `fleet` (its default; tags), `local` and `redis` (memory stores; no tags).

use Claviron\Runtime\CacheException;
use Claviron\Runtime\CacheTooManyTagsException;
use Illuminate\Cache\CacheManager;
use Illuminate\Config\Repository as Config;
use Illuminate\Foundation\Application;
use Lumenize\Cache\Store;
use Lumenize\Cache\TaggableStore;
use Lumenize\Providers\CacheServiceProvider;

set_exception_handler(static function (Throwable $error): void {
    http_response_code(500);
    echo $error;
});

function same(mixed $expected, mixed $actual, string $what): void
{
    if ($expected !== $actual) {
        throw new LogicException($what.': '.var_export([$expected, $actual], true));
    }
}

/** The exact class, so each refusal is told apart from its parent. */
function throws(string $class, callable $call, string $what): Throwable
{
    try {
        $call();
    } catch (Throwable $error) {
        same($class, $error::class, $what);

        return $error;
    }

    throw new LogicException("{$what}: expected {$class}");
}

// The harness runs this file more than once against the same runtime stores.
$prefix = 'run-'.bin2hex(random_bytes(6)).'-';
$app = new Application(sys_get_temp_dir());
$app->instance('config', new Config(['cache' => [
    'default' => 'shared',
    'prefix' => $prefix,
    'stores' => [
        // Laravel store names say nothing about runtime store names.
        'shared' => ['driver' => 'claviron', 'runtime_store' => 'fleet'],
        'remote' => ['driver' => 'claviron', 'runtime_store' => 'redis'],
        'fleet' => ['driver' => 'claviron', 'runtime_store' => 'local'],
        'runtime_default' => ['driver' => 'claviron'],
        'explicit_null' => ['driver' => 'claviron', 'runtime_store' => null],
        'other_app' => ['driver' => 'claviron', 'runtime_store' => 'fleet', 'prefix' => $prefix.'other-'],
        'missing' => ['driver' => 'claviron', 'runtime_store' => 'absent', 'fallback' => 'array'],
        'empty' => ['driver' => 'claviron', 'runtime_store' => ''],
        'blank' => ['driver' => 'claviron', 'runtime_store' => ' '],
        'numeric' => ['driver' => 'claviron', 'runtime_store' => 7],
        'array' => ['driver' => 'array'],
    ],
]]));
$cache = new CacheManager($app);
(new CacheServiceProvider($app))->boot($cache);
$raw = static fn (?string $store, string $key): mixed => claviron_cache_get(claviron_cache_store($store), $prefix.$key);

// Each Laravel store writes to the runtime store it names, and only there.
foreach (['shared' => 'fleet', 'remote' => 'redis', 'fleet' => 'local'] as $alias => $runtime) {
    same(true, $cache->store($alias)->put("selected:{$alias}", $alias, 60), "put through [{$alias}]");
    foreach (['fleet', 'redis', 'local'] as $candidate) {
        same($candidate === $runtime ? $alias : null, $raw($candidate, "selected:{$alias}"), "[{$alias}] in runtime store [{$candidate}]");
    }
}

// Omitted and null select the runtime default, which is `fleet` here.
foreach (['runtime_default', 'explicit_null'] as $alias) {
    $cache->store($alias)->put("default:{$alias}", $alias, 60);
    same($alias, $raw(null, "default:{$alias}"), "[{$alias}] in the runtime default");
    same($alias, $raw('fleet', "default:{$alias}"), "[{$alias}] in [fleet]");
}

// A name the runtime lacks is the runtime's error: no default, no fallback.
$error = throws(CacheException::class, fn () => $cache->store('missing'), 'missing runtime store');
same(true, str_contains($error->getMessage(), "'absent'"), 'missing store named in its error');
foreach (['empty', 'blank', 'numeric'] as $alias) {
    throws(InvalidArgumentException::class, fn () => $cache->store($alias), "invalid runtime_store in [{$alias}]");
}

// Values keep their encoding and Laravel's TTL rules, tagged or not.
$object = new ArrayObject(['id' => 7]);
$values = [
    'false' => false,
    'zero' => 0,
    'float' => 1.5,
    'string' => 'text',
    'marker' => "\0claviron:object\0literal",
    'array' => ['nested' => [1, 'two']],
];
foreach (['remote' => $cache->store('remote'), 'tagged' => $cache->store('shared')->tags(['encoding'])] as $label => $repository) {
    foreach ($values as $name => $value) {
        same(true, $repository->put("value:{$name}", $value, 60), "{$label} put {$name}");
        same($value, $repository->get("value:{$name}"), "{$label} read {$name}");
    }
    same(true, $repository->forever('value:object', $object), "{$label} forever");
    $read = $repository->get('value:object');
    same(true, $read == $object && $read !== $object, "{$label} object round trip");
    $repository->put('value:string', 'gone', 0);
    same(null, $repository->get('value:string'), "{$label} non-positive TTL forgets");
}
// Scalars and arrays stay native for other runtime readers.
same(['nested' => [1, 'two']], $raw('redis', 'value:array'), 'native array');
same(true, str_starts_with($raw('redis', 'value:object'), "\0claviron:object\0"), 'marked object');

// Tag support follows the resolved runtime store, not the driver or a name.
foreach (['shared' => true, 'runtime_default' => true, 'explicit_null' => true, 'other_app' => true, 'remote' => false, 'fleet' => false] as $alias => $expected) {
    $repository = $cache->store($alias);
    same($expected, $repository->supportsTags(), "supportsTags() of [{$alias}]");
    same($expected ? TaggableStore::class : Store::class, $repository->getStore()::class, "store of [{$alias}]");
    if (!$expected) {
        throws(BadMethodCallException::class, fn () => $repository->tags(['users']), "tags() on [{$alias}]");
        same(true, $repository->put('untagged', 'kept', 60), "untagged put on [{$alias}]");
        same('kept', $repository->get('untagged'), "untagged read on [{$alias}]");
    }
}

// An item is addressed through the ordered tag list it was stored with.
$shared = $cache->store('shared');
$shared->put('profile', 'untagged', 60);
$shared->tags(['users', 'team'])->put('profile', 'users+team', 60);
$shared->tags(['users'])->put('profile', 'users', 60);
$shared->tags(['team'])->put('profile', 'team', 60);
same('users+team', $shared->tags(['users', 'team'])->get('profile'), 'tag list read');
same(null, $shared->tags(['team', 'users'])->get('profile'), 'another order is another list');
same('untagged', $shared->get('profile'), 'untagged key beside tagged ones');

// Flushing one tag clears what every list containing it stored, nothing else.
$shared->tags(['users'])->flush();
same(null, $shared->tags(['users', 'team'])->get('profile'), 'flushed through a longer list');
same(null, $shared->tags(['users'])->get('profile'), 'flushed tag');
same('team', $shared->tags(['team'])->get('profile'), 'other tag kept');
same('untagged', $shared->get('profile'), 'untagged kept');
$shared->tags('users', 'team')->put('profile', 'again', 60);
same('again', $shared->tags(['users', 'team'])->get('profile'), 'a flushed tag takes new writes');

// Tags belong to the runtime store and the prefix: another Laravel store on
// both shares them, one with another prefix does not.
$other = $cache->store('other_app')->tags(['team']);
$other->put('profile', 'other', 60);
$cache->store('runtime_default')->tags(['team'])->flush();
same(null, $shared->tags(['team'])->get('profile'), 'flushed through the default store');
same(null, $shared->tags(['users', 'team'])->get('profile'), 'flushed list through the default store');
same('other', $other->get('profile'), 'another prefix keeps its tag');

// Repository operations through a tagged cache all write tagged.
$tagged = $cache->tags(['ops']);
same(true, $tagged->add('added', 'first', 60), 'add');
same(false, $tagged->add('added', 'second', 60), 'add over a value');
$calls = 0;
$compute = function () use (&$calls): string { ++$calls; return 'computed'; };
same('computed', $tagged->remember('remembered', 60, $compute), 'remember');
same('computed', $tagged->remember('remembered', 60, $compute), 'remember again');
same(1, $calls, 'remember computed once');
same('kept', $tagged->rememberForever('forever', fn () => 'kept'), 'rememberForever');
same(true, $tagged->putMany(['a' => 1, 'b' => 2], 60), 'putMany');
same(true, $tagged->putMany(['c' => 3]), 'putMany forever');
same(['a' => 1, 'b' => 2, 'c' => 3, 'z' => null], $tagged->many(['a', 'b', 'c', 'z']), 'many');
same(true, $tagged->touch('a', 120), 'touch');
same(1, $tagged->pull('a'), 'pull');
same(true, $tagged->forget('b'), 'forget');
same(false, $tagged->has('b'), 'forgotten');
$tagged->flush();
foreach (['added', 'remembered', 'forever', 'c'] as $key) {
    same(null, $tagged->get($key), "{$key} carried the tag");
}

// Counters keep no tags, so a tagged cache refuses them.
throws(BadMethodCallException::class, fn () => $tagged->increment('count'), 'tagged increment');
throws(BadMethodCallException::class, fn () => $tagged->decrement('count'), 'tagged decrement');

// Tag lists are checked where they are given.
throws(InvalidArgumentException::class, fn () => $shared->tags([]), 'no tags');
throws(TypeError::class, fn () => $shared->tags(['users', 7]), 'a tag that is not a string');
throws(ValueError::class, fn () => $shared->tags(["\xff"]), 'a tag that is not UTF-8');

// The runtime's bound reaches Laravel with its numbers; nothing is written.
$crowded = $shared->tags(array_map(fn (int $tag): string => "tag-{$tag}", range(1, 17)));
$error = throws(CacheTooManyTagsException::class, fn () => $crowded->put('crowded', 'value', 60), 'too many tags');
same(17, $error->count, 'tags counted');
same(true, is_int($error->max) && $error->max < 17, 'tags kept');
same(null, $crowded->get('crowded'), 'nothing written');
throws(CacheTooManyTagsException::class, fn () => $crowded->flush(), 'too many tags to invalidate');

header('X-Fixture: ok');
echo 'ok';
