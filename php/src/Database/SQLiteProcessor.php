<?php

declare(strict_types=1);

namespace Lumenize\Database;

use Illuminate\Database\Query\Processors\SQLiteProcessor as IlluminateSQLiteProcessor;

/** Keep SQLite schema processing; obtain the statement's ID without PDO. */
final class SQLiteProcessor extends IlluminateSQLiteProcessor
{
    public function processInsertGetId($query, $sql, $values, $sequence = null)
    {
        $connection = $query->getConnection();
        $connection->insert($sql, $values);
        $id = $connection->getLastInsertId();

        return is_numeric($id) ? (int) $id : $id;
    }
}
