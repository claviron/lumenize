<?php

declare(strict_types=1);

namespace Lumenize\Database;

use Closure;
use Illuminate\Database\SQLiteConnection as IlluminateSQLiteConnection;
use LogicException;
use PDO;
use RuntimeException;
use Throwable;

/** Laravel's SQLite grammar and Eloquent API over the runtime-owned pool. */
final class SQLiteConnection extends IlluminateSQLiteConnection
{
    private \Claviron\Runtime\SqliteConnection $nativeConnection;

    private ?string $lastInsertId = null;

    public function __construct(string $pool, string $database = '', string $prefix = '', array $config = [])
    {
        if (!function_exists('claviron_sqlite_connection')
            || !(new \ReflectionFunction('claviron_sqlite_connection'))->isInternal()
            || !function_exists('claviron_runtime_available')
            || !claviron_runtime_available()) {
            throw new RuntimeException('The Claviron SQLite runtime is unavailable; enable the sqlite feature.');
        }

        $this->nativeConnection = claviron_sqlite_connection($pool);
        parent::__construct(
            static fn (): never => throw new LogicException('Claviron SQLite has no PDO handle.'),
            $database, $prefix, $config,
        );
    }

    public function select($query, $bindings = [], $useReadPdo = true, array $fetchUsing = [])
    {
        if ($fetchUsing !== [] && $fetchUsing !== [PDO::FETCH_OBJ]) {
            throw new LogicException('Custom PDO fetch modes are unavailable on Claviron SQLite.');
        }

        return $this->run($query, $bindings, function ($query, $bindings): array {
            if ($this->pretending()) {
                return [];
            }

            $result = $this->executeNative($query, $bindings);
            $names = array_column($result['columns'], 'name');

            return array_map(static function (array $values) use ($names): object {
                $row = new \stdClass();
                foreach ($names as $index => $name) {
                    $row->{$name} = $values[$index] ?? null;
                }

                return $row;
            }, $result['rows']);
        });
    }

    public function selectResultSets($query, $bindings = [], $useReadPdo = true, array $fetchUsing = [])
    {
        throw new LogicException('Multiple SQLite result sets are not supported by the native adapter.');
    }

    /** Buffered results; no PDO statement escapes the native boundary. */
    public function cursor($query, $bindings = [], $useReadPdo = true, array $fetchUsing = [])
    {
        yield from $this->select($query, $bindings, $useReadPdo, $fetchUsing);
    }

    public function insert($query, $bindings = [], $sequence = null)
    {
        return $this->run($query, $bindings, function ($query, $bindings): bool {
            if ($this->pretending()) {
                return true;
            }
            $result = $this->executeNative($query, $bindings);
            $this->lastInsertId = $result['insert_id'];
            $this->recordsHaveBeenModified();

            return true;
        });
    }

    public function statement($query, $bindings = [])
    {
        return $this->run($query, $bindings, function ($query, $bindings): bool {
            if (!$this->pretending()) {
                $this->executeNative($query, $bindings);
                $this->recordsHaveBeenModified();
            }

            return true;
        });
    }

    public function affectingStatement($query, $bindings = [])
    {
        return $this->run($query, $bindings, function ($query, $bindings): int {
            if ($this->pretending()) {
                return 0;
            }
            $affected = (int) $this->executeNative($query, $bindings)['affected_rows'];
            $this->recordsHaveBeenModified($affected > 0);

            return $affected;
        });
    }

    public function unprepared($query)
    {
        return $this->run($query, [], function ($query): bool {
            if (!$this->pretending()) {
                self::native(fn () => claviron_sqlite_unprepared($this->nativeConnection, $query));
                $this->recordsHaveBeenModified();
            }

            return true;
        });
    }

    public function beginTransaction()
    {
        foreach ($this->beforeStartingTransaction as $callback) {
            $callback($this);
        }
        if ($this->transactionLevel() === 0) {
            self::native(fn () => claviron_sqlite_begin($this->nativeConnection, $this->getConfig('transaction_mode') ?? 'DEFERRED'));
        } else {
            $this->executeNative('SAVEPOINT trans'.($this->transactions + 1), []);
        }
        $this->transactions++;
        $this->transactionsManager?->begin($this->getName(), $this->transactions);
        $this->fireConnectionEvent('beganTransaction');
    }

    public function transaction(Closure $callback, $attempts = 1)
    {
        for ($attempt = 1; $attempt <= $attempts; $attempt++) {
            $this->beginTransaction();
            try {
                $result = $callback($this);
            } catch (Throwable $exception) {
                $this->handleTransactionException($exception, $attempt, $attempts);
                continue;
            }
            try {
                $this->commit();
            } catch (Throwable $exception) {
                $this->handleCommitTransactionException($exception, $attempt, $attempts);
                continue;
            }

            return $result;
        }

        throw new LogicException('Claviron SQLite transaction attempts exhausted.');
    }

    protected function handleCommitTransactionException(Throwable $e, $currentAttempt, $maxAttempts)
    {
        // Only a server-reported concurrency failure can be retried. An I/O
        // failure during COMMIT has an unknown outcome and must propagate.
        if ($this->transactions === 0) {
            // COMMIT succeeded; an afterCommit callback failed. Never replay it.
            throw $e;
        }
        $retry = $this->causedByConcurrencyError($e) && $currentAttempt < $maxAttempts;
        try {
            $this->rollBack(0);
        } catch (Throwable) {
            // Preserve the COMMIT error if cleanup also fails. Disconnect
            // closes the native lease rather than reusing uncertain state.
            $this->disconnect();
        }
        if ($retry) {
            return;
        }
        throw $e;
    }

    public function commit()
    {
        if ($this->transactions === 1) {
            $this->fireConnectionEvent('committing');
            self::native(fn () => claviron_sqlite_commit($this->nativeConnection));
        }
        [$level, $this->transactions] = [$this->transactions, max(0, $this->transactions - 1)];
        $this->transactionsManager?->commit($this->getName(), $level, $this->transactions);
        $this->fireConnectionEvent('committed');
    }

    public function rollBack($toLevel = null)
    {
        $toLevel ??= $this->transactions - 1;
        if ($toLevel < 0 || $toLevel >= $this->transactions) {
            return;
        }
        if (claviron_sqlite_in_transaction($this->nativeConnection)) {
            if ($toLevel === 0) {
                self::native(fn () => claviron_sqlite_rollback($this->nativeConnection));
            } else {
                $this->executeNative('ROLLBACK TO SAVEPOINT trans'.($toLevel + 1), []);
            }
        } else {
            $toLevel = 0;
        }
        $this->transactions = $toLevel;
        $this->transactionsManager?->rollback($this->getName(), $this->transactions);
        $this->fireConnectionEvent('rollingBack');
    }

    public function transactionLevel()
    {
        if ($this->transactions > 0 && !claviron_sqlite_in_transaction($this->nativeConnection)) {
            $this->transactions = 0;
            $this->transactionsManager?->rollback($this->getName(), 0);
        }

        return $this->transactions;
    }

    public function getPdo()
    {
        throw new LogicException('Claviron SQLite intentionally exposes no PDO handle.');
    }

    public function disconnect()
    {
        self::native(fn () => claviron_sqlite_disconnect($this->nativeConnection));
        $this->transactions = 0;
        $this->lastInsertId = null;
        $this->transactionsManager?->rollback($this->getName(), 0);
        // Keep the parent's no-PDO sentinel: this object names a pool, and
        // the next statement obtains a fresh native lease without a reconnector.
    }

    public function getLastInsertId(): ?string
    {
        return $this->lastInsertId;
    }

    public function getServerVersion(): string
    {
        return (string) $this->selectOne('SELECT sqlite_version() AS version')->version;
    }

    protected function getDefaultPostProcessor()
    {
        return new SQLiteProcessor;
    }

    protected function escapeString($value)
    {
        return "'".str_replace("'", "''", $value)."'";
    }

    private function executeNative(string $query, array $bindings): array
    {
        $bindings = $this->prepareBindings($bindings);
        foreach ($bindings as &$binding) {
            if (is_resource($binding)) {
                if (get_resource_type($binding) !== 'stream' || ($bytes = stream_get_contents($binding)) === false) {
                    throw new LogicException('SQLite LOB bindings require a readable stream.');
                }
                $binding = ['type' => 'bytes', 'value' => $bytes];
            }
        }
        unset($binding);

        return self::native(fn () => claviron_sqlite_execute(
            $this->nativeConnection, $query, $bindings,
        ));
    }

    private static function native(Closure $operation): mixed
    {
        try {
            return $operation();
        } catch (Throwable $failure) {
            throw QueryException::from($failure);
        }
    }
}
