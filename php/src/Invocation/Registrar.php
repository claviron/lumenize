<?php

declare(strict_types=1);

namespace Lumenize\Invocation;

/** Declares one application's handlers/events; never starts a consumer or loop. */
interface Registrar
{
    public function register(Registry $invocations): void;
}
