<?php

declare(strict_types=1);

namespace Lumenize\Http\Controllers;

use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Http\Response;
use LogicException;

/** Opt-in, stateless workloads for the persistent Laravel application path. */
final class BenchController
{
    public function plain(Request $request): Response
    {
        return new Response("ok\n", 200, $this->headers($request) + ['Content-Type' => 'text/plain']);
    }

    public function json(Request $request): JsonResponse
    {
        return new JsonResponse(['message' => 'ok'], 200, $this->headers($request));
    }

    public function echo(Request $request): Response
    {
        return new Response($request->getContent(), 200, $this->headers($request) + ['Content-Type' => 'application/octet-stream']);
    }

    public function state(Request $request): JsonResponse
    {
        $headers = $this->headers($request);

        return new JsonResponse(['mode' => 'application'] + $request->attributes->get('claviron'), 200, $headers);
    }

    private function headers(Request $request): array
    {
        $context = $request->attributes->get('claviron');
        if (! is_array($context)
            || ! is_string($context['boot_id'] ?? null)
            || ! is_int($context['request_count'] ?? null)
            || $context['request_count'] < 1
            || ($context['process_id'] ?? null) !== getmypid()) {
            throw new LogicException('Laravel benchmarks require the persistent Lumenize application client.');
        }

        return [
            'X-Lumenize-Mode' => 'application',
            'X-Lumenize-Boot-Id' => $context['boot_id'],
            'X-Lumenize-Request-Count' => (string) $context['request_count'],
            'X-Lumenize-Process-Id' => (string) $context['process_id'],
            'Cache-Control' => 'no-store',
        ];
    }
}
