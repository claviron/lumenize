<?php

declare(strict_types=1);

namespace Lumenize\Providers;

use Lumenize\Broadcasting\Broadcaster;
use Lumenize\Broadcasting\Events\ChannelOccupied;
use Lumenize\Broadcasting\Events\ChannelVacated;
use Lumenize\Broadcasting\UnconfiguredBroadcaster;
use Lumenize\EventBridge;
use Illuminate\Broadcasting\BroadcastManager;
use Illuminate\Broadcasting\Broadcasters\Broadcaster as IlluminateBroadcaster;
use Illuminate\Support\ServiceProvider;

final class BroadcastingProvider extends ServiceProvider
{
    public function boot(BroadcastManager $broadcast): void
    {
        $broadcast->extend('claviron', static function ($app, array $config): IlluminateBroadcaster {
            $key = (string) ($config['key'] ?? '');
            $secret = (string) ($config['secret'] ?? '');

            if ($key === '' || $secret === '') {
                return new UnconfiguredBroadcaster();
            }

            return new Broadcaster(
                $key,
                $secret,
                (bool) ($config['require_authenticated_user'] ?? true),
            );
        });

        if (EventBridge::available()) {
            $this->app->make(EventBridge::class)->subscribe([
                ChannelOccupied::class,
                ChannelVacated::class,
            ]);
        }
    }
}
