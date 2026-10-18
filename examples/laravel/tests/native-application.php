<?php

declare(strict_types=1);

require __DIR__.'/../vendor/autoload.php';

// An opt-in executable harness. The ordinary example never loads these checks.
Lumenize\Octane\ApplicationRunner::run(dirname(__DIR__), function ($app): string {
    $db = $app['db']->connection();
    if (!$db instanceof Lumenize\Database\SQLiteConnection) {
        throw new RuntimeException('Expected the native SQLite adapter, not PDO.');
    }
    $check = static function (bool $condition, string $message): void {
        if (!$condition) {
            throw new RuntimeException($message);
        }
    };
    $handle = claviron_sqlite_connection('example');
    $callbacks = 0;
    try {
        $db->beginTransaction();
        $db->statement('CREATE TEMP TABLE native_sqlite_probe (id INTEGER PRIMARY KEY, title TEXT UNIQUE, content BLOB)');
        $model = new class extends Illuminate\Database\Eloquent\Model {
            protected $table = 'native_sqlite_probe';
            protected $guarded = [];
            public $timestamps = false;
        };
        $model->setConnection($db->getName());
        $entry = $model->newQuery()->create(['title' => 'outer']);
        $check($entry->id === 1, 'Eloquent insert ID was not supplied by SQLite.');
        claviron_sqlite_execute($handle, 'UPDATE native_sqlite_probe SET content = ? WHERE id = ?', [
            ['type' => 'bytes', 'value' => "\x00\xff"], $entry->id,
        ]);
        $check($entry->fresh()->content === "\x00\xff", 'The PHP/Rust BLOB boundary changed bytes.');

        $db->beginTransaction();
        $model->newQuery()->create(['title' => 'inner']);
        $db->rollBack();
        $check($model->newQuery()->count() === 1 && $db->transactionLevel() === 1, 'Nested rollback escaped its savepoint.');
        try {
            $model->newQuery()->create(['title' => 'outer']);
            throw new RuntimeException('The unique violation was ignored.');
        } catch (Illuminate\Database\UniqueConstraintViolationException $failure) {
            $check($failure->errorInfo[0] === '23000' && $failure->errorInfo[1] === 2067, 'SQLite constraint details were lost.');
        }
        $check($model->newQuery()->count() === 1, 'A constraint error aborted an ordinary SQLite transaction.');
        $db->afterCommit(function () use (&$callbacks): void { $callbacks++; });
        $db->commit();
        $check($callbacks === 1, 'afterCommit did not run after the outer commit.');

        $db->beginTransaction();
        $db->statement('CREATE TEMP TABLE native_sqlite_abort_probe (id INTEGER PRIMARY KEY)');
        $db->statement('INSERT INTO native_sqlite_abort_probe VALUES (1)');
        try {
            $db->statement('INSERT OR ROLLBACK INTO native_sqlite_abort_probe VALUES (1)');
            throw new RuntimeException('ON CONFLICT ROLLBACK did not fail.');
        } catch (Illuminate\Database\UniqueConstraintViolationException) {
            // SQLite rolled back the entire transaction, not just the statement.
        }
        try {
            $db->select('SELECT 1');
            throw new RuntimeException('An aborted transaction silently resumed autocommit.');
        } catch (Illuminate\Database\QueryException $failure) {
            $check(str_contains($failure->getMessage(), 'session failed'), 'Unexpected aborted-session failure.');
        }
        $db->rollBack();
        $check($db->transactionLevel() === 0, 'Rollback did not clear the aborted scope.');
        $check($db->selectOne('SELECT 1 AS value')->value === 1, 'The adapter could not start fresh after rollback.');
    } finally {
        $db->disconnect();
    }

    error_log('PASS: native SQLite, Eloquent, BLOBs, nested rollback, constraints, afterCommit and aborted-session recovery');
    $app['router']->get('/_native/sqlite/abandon', function () use ($db) {
        $db->beginTransaction();
        $db->statement('CREATE TEMP TABLE native_sqlite_abandoned (id INTEGER PRIMARY KEY)');
        $db->statement('INSERT INTO native_sqlite_abandoned VALUES (1)');

        // Deliberately leave this transaction to the native message boundary.
        return response()->json(['abandoned' => $db->transactionLevel() === 1]);
    });
    $app['router']->get('/_native/sqlite/clean', function () use ($db, $check) {
        $check($db->transactionLevel() === 0, 'A transaction survived its HTTP request.');
        $row = $db->selectOne("SELECT COUNT(*) AS total FROM sqlite_temp_schema WHERE name = 'native_sqlite_abandoned'");
        $check($row->total === 0, 'A temporary table survived its native lease.');

        return response()->json(['clean' => true]);
    });
    return Illuminate\Http\Request::class;
});
