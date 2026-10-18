<?php

declare(strict_types=1);

namespace Lumenize\Auth;

use Illuminate\Contracts\Auth\Authenticatable;

/** Keeps Eloquent boot/casts/retrieved semantics without a second SELECT. */
class EloquentUserProvider extends \Illuminate\Auth\EloquentUserProvider implements NativeUserProvider
{
    public function hydrateFromNative(array $attributes): Authenticatable
    {
        return $this->createModel()->newFromBuilder($attributes);
    }
}
