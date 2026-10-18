<?php

declare(strict_types=1);

use App\Models\Note;
use Illuminate\Contracts\Console\Kernel as ConsoleKernel;
use Illuminate\Contracts\Http\Kernel as HttpKernel;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Artisan;
use Illuminate\Support\Facades\DB;

require __DIR__.'/../vendor/autoload.php';

$database = tempnam(sys_get_temp_dir(), 'lumenize-laravel-');
if ($database === false) {
    throw new RuntimeException('Cannot create the isolated SQLite fixture.');
}

// These overrides belong only to this PHP process, never the example's .env.
foreach ([
    'APP_ENV' => 'local',
    'APP_KEY' => 'base64:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=',
    'DB_CONNECTION' => 'sqlite',
    'DB_DATABASE' => $database,
    'SESSION_DRIVER' => 'array',
    'CACHE_STORE' => 'array',
] as $key => $value) {
    putenv($key.'='.$value);
    $_ENV[$key] = $_SERVER[$key] = $value;
}

function check(bool $condition, string $message): void
{
    if (! $condition) {
        throw new RuntimeException($message);
    }
}

$app = null;
try {
    check(realpath(__DIR__.'/../vendor/claviron/laravel') === realpath(dirname(__DIR__, 3).'/php'), 'The PHP integration must be the local path package.');

    $app = require __DIR__.'/../bootstrap/app.php';
    $app->make(ConsoleKernel::class)->bootstrap();
    check(Artisan::call('migrate', ['--database' => 'sqlite', '--force' => true]) === 0, 'Migration failed.');
    check(Artisan::call('db:seed', ['--force' => true]) === 0, 'Seeding failed.');
    check(Artisan::call('db:seed', ['--force' => true]) === 0, 'Repeated seeding failed.');
    check(Note::count() === 2, 'The seed data must be repeatable.');
    check(DB::connection()->getPdo()->getAttribute(PDO::ATTR_DRIVER_NAME) === 'sqlite', 'PHP must use PDO SQLite.');

    $kernel = $app->make(HttpKernel::class);
    $cookies = [];
    $request = function (string $method, string $path, array $data = []) use ($kernel, &$cookies) {
        $request = Request::create('http://localhost'.$path, $method, $data, $cookies, [], ['HTTP_ACCEPT' => 'application/json']);
        $response = $kernel->handle($request);
        foreach ($response->headers->getCookies() as $cookie) {
            $cookies[$cookie->getName()] = $cookie->getValue();
        }
        $kernel->terminate($request, $response);

        return $response;
    };

    $home = $request('GET', '/');
    check($home->getStatusCode() === 200, 'The notes page must render.');
    check(str_contains($home->getContent(), 'Open the Laravel example'), 'The page must show SQLite seed data.');
    check(preg_match('/name="_token" value="([^"]+)"/', $home->getContent(), $match) === 1, 'The form must contain a CSRF token.');
    $token = $match[1];

    check($request('POST', '/notes', ['_token' => 'invalid', 'title' => 'Rejected'])->getStatusCode() === 419, 'Invalid CSRF must be rejected.');
    $invalid = $request('POST', '/notes', ['_token' => $token, 'title' => '']);
    check($invalid->getStatusCode() === 422, 'An empty title must fail validation.');
    check(isset(json_decode($invalid->getContent(), true)['errors']['title']), 'Validation must identify the title field.');
    check(Note::count() === 2, 'Rejected requests must not create notes.');

    $title = '<script>not executable</script>';
    check($request('POST', '/notes', ['_token' => $token, 'title' => $title])->getStatusCode() === 302, 'Creating a note must redirect.');
    $note = Note::query()->where('title', $title)->firstOrFail();
    $html = $request('GET', '/')->getContent();
    check(str_contains($html, '&lt;script&gt;not executable&lt;/script&gt;'), 'Blade must escape note titles.');
    check(! str_contains($html, $title), 'Unescaped user content must not be rendered.');

    check($request('PATCH', '/notes/'.$note->id, ['_token' => $token, 'completed' => '1'])->getStatusCode() === 302, 'Updating a note must redirect.');
    check($note->fresh()->completed, 'The update must reach SQLite.');
    check($request('DELETE', '/notes/'.$note->id, ['_token' => $token])->getStatusCode() === 302, 'Deleting a note must redirect.');
    check(Note::count() === 2, 'The deletion must reach SQLite.');

    echo "Laravel example passed: local package, SQLite, page, CSRF, validation, escaping and note changes.\n";
} finally {
    if ($app !== null) {
        $app->make('db')->disconnect('sqlite');
    }
    foreach ([$database, $database.'-wal', $database.'-shm'] as $path) {
        if (is_file($path)) {
            unlink($path);
        }
    }
}
