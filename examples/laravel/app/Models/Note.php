<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

final class Note extends Model
{
    protected $fillable = ['title', 'completed'];

    protected function casts(): array
    {
        return ['completed' => 'boolean'];
    }
}
