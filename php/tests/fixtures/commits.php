<?php

declare(strict_types=1);

namespace Lumenize\Database;

use Illuminate\Database\Connection;
use Illuminate\Database\Events\TransactionCommitted;
use Illuminate\Events\Dispatcher;
use RuntimeException;
use Throwable;

/** Shared checks using each adapter suite's substituted native Backend. */
function checkCommitBoundaries(Connection $db): void
{
    // Callback code may start another transaction before failing. The new
    // nesting count cannot be used as evidence that the original COMMIT failed.
    foreach ([false, true] as $startsTransaction) {
        $db->disconnect();
        Backend::$calls = [];
        $runs = 0;
        $failure = new RuntimeException('Deadlock found when trying to get lock: afterCommit');
        $caught = null;
        try {
            $db->transaction(function ($connection) use (&$runs, $failure, $startsTransaction): void {
                ++$runs;
                $connection->afterCommit(static function () use ($connection, $failure, $startsTransaction): void {
                    if ($startsTransaction) {
                        $connection->beginTransaction();
                    }
                    throw $failure;
                });
            }, 3);
        } catch (Throwable $error) {
            $caught = $error;
        }
        check($caught === $failure, 'The original afterCommit failure was not propagated');
        check($runs === 1, 'A successful COMMIT was replayed after its callback failed');
        check(count(array_filter(Backend::$calls, fn ($call) => $call[0] === 'COMMIT')) === 1, 'A callback failure caused another COMMIT');
        check($db->transactionLevel() === (int) $startsTransaction, 'Post-commit failure changed callback-owned transaction state');
        $db->rollBack();
    }

    $db->disconnect();
    Backend::$calls = [];
    $runs = 0;
    $failure = new RuntimeException('Deadlock found when trying to get lock: committed event');
    $events = new Dispatcher();
    $events->listen(TransactionCommitted::class, static function () use ($failure): void { throw $failure; });
    $db->setEventDispatcher($events);
    $caught = null;
    try {
        $db->transaction(function () use (&$runs): void { ++$runs; }, 3);
    } catch (Throwable $error) {
        $caught = $error;
    } finally {
        $db->unsetEventDispatcher();
    }
    check($caught === $failure && $runs === 1, 'A committed event failure replayed completed work or lost its exception');
    check(count(array_filter(Backend::$calls, fn ($call) => $call[0] === 'COMMIT')) === 1, 'A committed event failure caused another COMMIT');
    check($db->transactionLevel() === 0, 'A committed event failure changed the completed transaction level');

    // A server-reported deadlock before COMMIT still retries the closure.
    $db->disconnect();
    Backend::$calls = [];
    Backend::$failure = 'Deadlock found when trying to get lock';
    $runs = 0;
    $result = $db->transaction(function ($connection) use (&$runs): string {
        ++$runs;
        $connection->statement('UPDATE entries SET credit = ?', ['0.000001']);
        return 'done';
    }, 2);
    check($result === 'done' && $runs === 2, 'A transaction-body deadlock no longer retries');
    check(count(array_filter(Backend::$calls, fn ($call) => $call[0] === 'COMMIT')) === 1, 'A rejected transaction body committed');

    // A genuine COMMIT rejection uses the existing rollback/retry path.
    $db->disconnect();
    Backend::$calls = [];
    Backend::$commitFailure = 'Deadlock found when trying to get lock';
    $runs = 0;
    $result = $db->transaction(function () use (&$runs): string { ++$runs; return 'committed'; }, 2);
    check($result === 'committed' && $runs === 2, 'A server-reported COMMIT deadlock no longer retries');
    check(count(array_filter(Backend::$calls, fn ($call) => $call[0] === 'COMMIT')) === 2, 'The failed COMMIT was not retried once');
    check(count(array_filter(Backend::$calls, fn ($call) => $call[0] === 'ROLLBACK')) === 1, 'The failed COMMIT retained its open native transaction');
    check($db->transactionLevel() === 0 && !Backend::$transaction, 'Successful retry retained transaction state');

    $db->disconnect();
    Backend::$calls = [];
    Backend::$failCommit = true;
    $runs = 0;
    $caught = null;
    try {
        $db->transaction(function () use (&$runs): void { ++$runs; }, 3);
    } catch (Throwable $error) {
        $caught = $error;
    } finally {
        Backend::$failCommit = false;
    }
    check($caught !== null && $runs === 1, 'An ambiguous COMMIT outcome was retried');
    check(count(array_filter(Backend::$calls, fn ($call) => $call[0] === 'COMMIT')) === 1, 'An ambiguous COMMIT was repeated');
    $db->disconnect();
}
