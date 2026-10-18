<?php

declare(strict_types=1);

// Native runtime presence without a configured event bus must not subscribe.
return \Illuminate\Foundation\Application::configure(basePath: dirname(__DIR__))
    ->withProviders([
        \Lumenize\Providers\LumenizeProvider::class,
        \Lumenize\Providers\ScheduleServiceProvider::class,
        \Lumenize\Providers\BroadcastingProvider::class,
    ], withBootstrapProviders: false)
    ->withExceptions(static function (\Illuminate\Foundation\Configuration\Exceptions $exceptions): void {})
    ->create();
