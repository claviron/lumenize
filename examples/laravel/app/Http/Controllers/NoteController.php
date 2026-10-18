<?php

namespace App\Http\Controllers;

use App\Models\Note;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\View\View;

final class NoteController
{
    public function index(): View
    {
        return view('notes', ['notes' => Note::query()->orderBy('id')->get()]);
    }

    public function store(Request $request): RedirectResponse
    {
        Note::query()->create($request->validate([
            'title' => ['required', 'string', 'max:160'],
        ]));

        return redirect()->route('notes.index');
    }

    public function update(Request $request, Note $note): RedirectResponse
    {
        $note->update($request->validate(['completed' => ['required', 'boolean']]));

        return redirect()->route('notes.index');
    }

    public function destroy(Note $note): RedirectResponse
    {
        $note->delete();

        return redirect()->route('notes.index');
    }
}
