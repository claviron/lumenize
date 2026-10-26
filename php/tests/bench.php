<?php

declare(strict_types=1);

// Exercise the real Octane client conversion without starting a host or DB.
require $argv[1];

use Illuminate\Http\Request;
use Laravel\Octane\RequestContext;
use Lumenize\Http\Controllers\BenchController;
use Lumenize\Octane\ClavironClient;

$controller = new BenchController();
$client = new ClavironClient(static function (array $response): void {});
$last = null;
for ($count = 1; $count <= 2; $count++) {
    [$request] = $client->marshalRequest(new RequestContext(['message' => [
        'method' => 'GET', 'uri' => 'https://example.test/bench/laravel/plain',
        'headers' => [], 'body' => '', 'server' => [], 'protocol' => 'HTTP/1.1',
    ]]));
    $response = $controller->plain($request);
    if ($response->getContent() !== "ok\n"
        || $response->headers->get('X-Lumenize-Mode') !== 'application'
        || $response->headers->get('X-Lumenize-Request-Count') !== (string) $count
        || ($last !== null && $last !== $response->headers->get('X-Lumenize-Boot-Id'))) {
        throw new LogicException('Persistent benchmark context was not preserved.');
    }
    $last = $response->headers->get('X-Lumenize-Boot-Id');
}
try {
    $controller->plain(new Request());
    throw new LogicException('A script request was accepted as persistent.');
} catch (LogicException $error) {
    if (! str_contains($error->getMessage(), 'require the persistent')) {
        throw $error;
    }
}
echo "Persistent Laravel benchmark identity and script rejection passed.\n";
