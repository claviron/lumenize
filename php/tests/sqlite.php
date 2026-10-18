<?php

declare(strict_types=1);

namespace {
    // php tests/sqlite.php /path/to/a/consumer/vendor/autoload.php
    require $argv[1];
}

namespace Claviron\Runtime {
    if (!class_exists(SqliteConnection::class, false)) {
        final class SqliteConnection {}
    }
}

namespace Lumenize\Database {
    use Illuminate\Database\Connection;
    use Illuminate\Database\DatabaseTransactionsManager;
    use Illuminate\Database\UniqueConstraintViolationException;
    use RuntimeException;

    final class Backend
    {
        public static bool $transaction = false;
        public static array $calls = [];
        public static ?string $failure = null;
        public static bool $failCommit = false;
    }

    function claviron_sqlite_execute($handle, $sql, $parameters = []): array
    {
        Backend::$calls[] = [$sql, $parameters];
        if ($failure = Backend::$failure) {
            Backend::$failure = null;
            throw new RuntimeException($failure);
        }
        return [
            'columns' => [['name' => 'credit'], ['name' => 'content']],
            'rows' => [['123456789012345678.123456', "\x00\xff"]],
            'affected_rows' => 2, 'insert_id' => '41',
        ];
    }
    function claviron_sqlite_begin($handle, $mode = 'DEFERRED'): bool
    {
        Backend::$calls[] = ['BEGIN'];
        return Backend::$transaction = true;
    }
    function claviron_sqlite_commit($handle): bool
    {
        Backend::$calls[] = ['COMMIT'];
        Backend::$transaction = false;
        if (Backend::$failCommit) {
            throw new RuntimeException('SQLSTATE[HY000]: General error: 10 disk I/O error');
        }
        return true;
    }
    function claviron_sqlite_rollback($handle): bool
    {
        Backend::$calls[] = ['ROLLBACK'];
        Backend::$transaction = false;
        return true;
    }
    function claviron_sqlite_in_transaction($handle): bool { return Backend::$transaction; }
    function claviron_sqlite_disconnect($handle): bool
    {
        Backend::$transaction = false;
        Backend::$calls[] = ['DISCONNECT'];
        return true;
    }

    function check(bool $condition, string $message): void
    {
        if (!$condition) { throw new RuntimeException($message); }
    }

    // The test replaces only the native boundary, not Laravel's transaction
    // manager, query pipeline, grammar, processor or exception conversion.
    $reflection = new \ReflectionClass(SQLiteConnection::class);
    $db = $reflection->newInstanceWithoutConstructor();
    $reflection->getProperty('nativeConnection')->setValue($db, (new \ReflectionClass(\Claviron\Runtime\SqliteConnection::class))->newInstanceWithoutConstructor());
    (new \ReflectionMethod(Connection::class, '__construct'))->invoke(
        $db, fn () => throw new RuntimeException('PDO must never be accessed'),
        'fixture', '', ['name' => 'sqlite', 'driver' => 'sqlite'],
    );
    $db->setTransactionManager(new DatabaseTransactionsManager());
    check($db->getName() === 'sqlite', 'Eloquent must retain its connection name');

    $row = $db->selectOne('SELECT ?, "?"', ["\x00\xff"]);
    check($row->credit === '123456789012345678.123456', 'TEXT must retain its value');
    check($row->content === "\x00\xff", 'BLOB must preserve bytes');
    check(Backend::$calls[0] === ['SELECT ?, "?"', ["\x00\xff"]], 'SQLite placeholders/binary binding changed');
    check($db->table('entries')->insertGetId(['credit' => '0.000001']) === 41, 'insertGetId bypassed native insert ID');
    $stream = fopen('php://memory', 'w+');
    fwrite($stream, "\x00\xff");
    rewind($stream);
    $db->statement('INSERT INTO entries(content) VALUES (?)', [$stream]);
    check(end(Backend::$calls)[1] === [['type' => 'bytes', 'value' => "\x00\xff"]], 'LOB stream did not become a binary binding');
    fclose($stream);

    Backend::$calls = [];
    $callbacks = 0;
    $db->beginTransaction();
    $db->beginTransaction();
    $db->afterCommit(function () use (&$callbacks) { $callbacks++; });
    $db->commit();
    check($callbacks === 0 && $db->transactionLevel() === 1, 'inner commit committed the cashier transaction');
    $db->commit();
    check($callbacks === 1 && Backend::$calls === [['BEGIN'], ['SAVEPOINT trans2', []], ['COMMIT']], 'outer commit contract');

    Backend::$calls = [];
    $db->beginTransaction();
    $db->beginTransaction();
    $db->rollBack(1);
    check($db->transactionLevel() === 1 && Backend::$calls[2][0] === 'ROLLBACK TO SAVEPOINT trans2', 'nested rollback');
    $db->rollBack();
    check(!Backend::$transaction && $db->transactionLevel() === 0, 'outer rollback');

    $runs = 0;
    try {
        $db->transaction(function () use (&$runs) { $runs++; throw new RuntimeException('business rule'); }, 3);
    } catch (RuntimeException $error) {
        check($error->getMessage() === 'business rule', 'original business failure lost');
    }
    check($runs === 1 && !Backend::$transaction, 'business errors must not retry');

    $runs = 0;
    $db->transaction(function ($db) use (&$runs) {
        if (++$runs === 1) {
            Backend::$failure = 'SQLSTATE[HY000]: General error: 5 database is locked';
        }
        $db->statement('UPDATE entries SET credit = ?', ['0.000001']);
    }, 2);
    check($runs === 2, 'SQLite busy failures must retain Laravel retry semantics');

    Backend::$failure = 'SQLSTATE[23000]: Integrity constraint violation: 2067 UNIQUE constraint failed: entries.name';
    try {
        $db->statement('INSERT INTO entries VALUES (?)', [1]);
        throw new RuntimeException('duplicate key must throw');
    } catch (UniqueConstraintViolationException $error) {
        check($error->errorInfo[1] === 2067, 'SQLite extended error code lost');
    }

    $runs = 0;
    Backend::$failCommit = true;
    try {
        $db->transaction(function () use (&$runs) { $runs++; }, 3);
        throw new RuntimeException('ambiguous commit must throw');
    } catch (QueryException $error) {
        check(str_contains($error->getMessage(), 'disk I/O error'), 'commit failure was masked');
    }
    check($runs === 1, 'ambiguous COMMIT must not replay the cashier operation');
    Backend::$failCommit = false;

    $runs = 0;
    try {
        $db->transaction(function ($db) use (&$runs) {
            $runs++;
            $db->afterCommit(static function (): void {
                throw new RuntimeException('database is locked: failure in an afterCommit callback');
            });
        }, 3);
        throw new RuntimeException('The failing afterCommit callback must propagate.');
    } catch (RuntimeException $error) {
        check(str_contains($error->getMessage(), 'afterCommit callback'), 'afterCommit failure was masked');
    }
    check($runs === 1, 'A successful COMMIT was replayed because its callback failed');

    $db->beginTransaction();
    Backend::$transaction = false; // runtime ended an abandoned work unit
    check($db->transactionLevel() === 0, 'stale Octane transaction count survived');
    check($db->escape("a'\\b") === "'a''\\b'", 'SQLite quote escaping changed');
    $db->beginTransaction();
    $db->disconnect();
    check(!$db->transactionLevel() && $db->getLastInsertId() === null, 'disconnect retained request state');
    check($db->selectOne('SELECT 1')->credit === '123456789012345678.123456', 'disconnected adapter tried to reconnect PDO');
    echo "PASS: SQLite Laravel adapter, native IDs/types, nested transactions, callbacks, retries and ambiguous commit\n";
}
