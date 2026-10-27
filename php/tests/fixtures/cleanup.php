<?php

declare(strict_types=1);

namespace Lumenize\Database;

use Illuminate\Database\Connection;
use Illuminate\Database\DatabaseTransactionsManager;

/** Native cleanup rolls back the old work before PHP opens another unit. */
function checkNativeTransactionCleanup(Connection $db): void
{
    foreach ([false, true] as $nested) {
        foreach ([false, true] as $explicitReconciliation) {
            $db->disconnect();
            $manager = new DatabaseTransactionsManager();
            $db->setTransactionManager($manager);
            $oldCallbacks = 0;
            $newCallbacks = 0;
            $otherCallbacks = 0;
            $manager->begin('other', 1);
            $manager->addCallback(function () use (&$otherCallbacks): void { ++$otherCallbacks; });

            $db->beginTransaction();
            $db->afterCommit(function () use (&$oldCallbacks): void { ++$oldCallbacks; });
            if ($nested) {
                $db->beginTransaction();
                $db->afterCommit(function () use (&$oldCallbacks): void { ++$oldCallbacks; });
                $db->commit(); // Stage nested callbacks; the root is still open.
            }
            Backend::$transaction = false;
            Backend::$calls = [];
            if ($explicitReconciliation) {
                check($db->transactionLevel() === 0, 'Native cleanup retained the old transaction level');
                check($db->transactionLevel() === 0, 'Repeated cleanup reconciliation is not idempotent');
                check(Backend::$calls === [], 'Reconciliation issued SQL after native cleanup already rolled back');
            }

            // Without the explicit check, beginTransaction must reconcile
            // the abandoned work itself before registering this fresh unit.
            $db->beginTransaction();
            $db->afterCommit(function () use (&$newCallbacks): void { ++$newCallbacks; });
            check($db->transactionLevel() === 1, 'Reconciliation damaged the new active transaction');
            $db->commit();
            check($oldCallbacks === 0, 'Native cleanup retained callbacks for rolled-back work');
            check($newCallbacks === 1, 'Native cleanup discarded a new transaction callback');
            check($otherCallbacks === 0, 'Commit ran callbacks belonging to another connection');
            $manager->commit('other', 1, 0);
            check($otherCallbacks === 1, 'Native cleanup discarded another connection callback');

            $db->beginTransaction();
            $db->afterCommit(function () use (&$newCallbacks): void { ++$newCallbacks; });
            $db->commit();
            check($oldCallbacks === 0 && $newCallbacks === 2, 'Abandoned callbacks leaked into later work');
        }
    }
    $db->disconnect();
}
