<?php

return [
    'default' => env('LOG_CHANNEL', 'single'),
    'channels' => [
        'single' => [
            'driver' => 'single',
            'path' => storage_path('logs/laravel.log'),
            'level' => 'debug',
        ],
    ],
];
