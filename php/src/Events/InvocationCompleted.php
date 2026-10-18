<?php

declare(strict_types=1);

namespace Lumenize\Events;

/** An invoked program ended with exit code zero. */
final class InvocationCompleted extends InvocationOutcome
{
    public static function topic(): string
    {
        return 'InvocationCompleted';
    }
}
