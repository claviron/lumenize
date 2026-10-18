<?php

namespace Database\Seeders;

use App\Models\Note;
use Illuminate\Database\Seeder;

final class DatabaseSeeder extends Seeder
{
    public function run(): void
    {
        foreach (['Open the Laravel example', 'Add a note with PHP and SQLite'] as $title) {
            Note::query()->firstOrCreate(['title' => $title]);
        }
    }
}
