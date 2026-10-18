<?php

declare(strict_types=1);

namespace Lumenize\Broadcasting\Events;

/** The last subscriber vacated a Pusher channel in this application. */
final class ChannelVacated extends ChannelOccupancy
{
    public static function topic(): string
    {
        return 'channel_vacated';
    }
}
