<?php

declare(strict_types=1);

namespace Lumenize\Cache;

use Illuminate\Cache\TagSet as IlluminateTagSet;

/**
 * Laravel's tag set over runtime tags.
 *
 * Laravel keeps a random identifier per tag in the cache and namespaces keys
 * by those identifiers, so a flush changes the namespace and strands what was
 * written under the old one. Here the runtime invalidates by tag, so the
 * namespace is the ordered tag names themselves and never changes; a flush
 * invalidates the tags instead, and nothing is stored to identify a tag.
 */
final class TagSet extends IlluminateTagSet
{
    private readonly string $namespace;

    /**
     * @param  list<string>  $names
     */
    public function __construct(TagScope $scope, array $names)
    {
        parent::__construct($scope, $names);

        // Length-prefixed, so no tag name can make two lists read alike.
        $namespace = '';
        foreach ($names as $name) {
            $namespace .= strlen($name).':'.$name;
        }
        $this->namespace = $namespace;
    }

    public function getNamespace()
    {
        return $this->namespace;
    }

    public function tagId($name)
    {
        return $name;
    }

    public function reset()
    {
        $this->store->invalidate();
    }

    public function flush()
    {
        $this->store->invalidate();
    }

    public function resetTag($name)
    {
        $this->store->invalidate([$name]);

        return $name;
    }

    public function flushTag($name)
    {
        $this->store->invalidate([$name]);
    }
}
