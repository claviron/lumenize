<?php

declare(strict_types=1);

// Autoload ownership only: no application boot, native runtime or queue access.
$loader = require $argv[1] ?? throw new LogicException('Supply a Composer autoloader');

$prefixes = $loader->getPrefixesPsr4();
if (!isset($prefixes['Lumenize\\'], $prefixes['Claviron\\'])
    || isset($prefixes['Claviron\\Laravel\\'])) {
    throw new LogicException('Integration and engine autoload ownership are not separate.');
}

$package = json_decode(file_get_contents(__DIR__.'/../composer.json'), true, flags: JSON_THROW_ON_ERROR);
foreach ($package['extra']['laravel']['providers'] as $provider) {
    if (!str_starts_with($provider, 'Lumenize\\') || !class_exists($provider)) {
        throw new LogicException('A discovered integration provider cannot be autoloaded.');
    }
}

foreach ([
    Lumenize\Auth\Events\AuthEvent::class,
    Lumenize\EventBridge::class,
    Lumenize\Invocation\Registry::class,
    Lumenize\Http\Middleware\StartNativeSession::class,
] as $class) {
    if (!class_exists($class)) {
        throw new LogicException('An integration class cannot be autoloaded.');
    }
    $former = 'Claviron\\Laravel\\'.substr($class, strlen('Lumenize\\'));
    if (class_exists($former)) {
        throw new LogicException('An old-namespace alias remains installed.');
    }
}

foreach ([
    Claviron\Runtime\Event::class,
    Claviron\Runtime\Invocation::class,
    Claviron\Runtime\Orbital::class,
    Claviron\Monolog\ClavironHandler::class,
] as $class) {
    if (!class_exists($class) || (new ReflectionClass($class))->getName() !== $class) {
        throw new LogicException('An engine-owned class was renamed or lost.');
    }
}

echo "Lumenize integration autoload, provider discovery and Claviron engine ownership verified.\n";
