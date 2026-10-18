<!doctype html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Lumenize · Laravel notes</title>
    <style>
        body { font: 1rem/1.6 system-ui, sans-serif; background: #f5f6f8; color: #202839; margin: 0; }
        main { max-width: 42rem; margin: 4rem auto; padding: 2rem; background: white; border-radius: 1rem; }
        h1 { margin: 0; } p { color: #596478; } label { display: block; font-weight: 600; }
        input, button { font: inherit; padding: .5rem .75rem; border: 1px solid #b3bdcb; border-radius: .4rem; }
        input { min-width: 0; flex: 1; } button { cursor: pointer; background: #eef2ff; }
        .entry, li { display: flex; gap: .5rem; align-items: center; } .entry { margin-top: .5rem; }
        ul { padding: 0; list-style: none; } li { padding: .75rem 0; border-bottom: 1px solid #e6e9ef; }
        li span { flex: 1; overflow-wrap: anywhere; } .completed { text-decoration: line-through; color: #6b7280; }
        .error { color: #a21b2b; margin: .25rem 0; } small { color: #596478; }
        @media (max-width: 48rem) { main { margin: 1rem; padding: 1rem; } }
    </style>
</head>
<body>
<main>
    <h1>Laravel notes</h1>
    <p>Ordinary Laravel forms, validation and Eloquent, running inside Lumenize.</p>

    <form method="post" action="{{ route('notes.store') }}">
        @csrf
        <label for="title">New note</label>
        <div class="entry">
            <input id="title" name="title" value="{{ old('title') }}" maxlength="160" required
                   @error('title') aria-invalid="true" aria-describedby="title-error" @enderror>
            <button type="submit">Add</button>
        </div>
        @error('title') <p class="error" id="title-error">{{ $message }}</p> @enderror
    </form>

    <ul>
        @forelse ($notes as $note)
            <li>
                <span @class(['completed' => $note->completed])>{{ $note->title }}</span>
                <form method="post" action="{{ route('notes.update', $note) }}">
                    @csrf
                    @method('PATCH')
                    <input type="hidden" name="completed" value="{{ $note->completed ? 0 : 1 }}">
                    <button type="submit">{{ $note->completed ? 'Reopen' : 'Done' }}</button>
                </form>
                <form method="post" action="{{ route('notes.destroy', $note) }}">
                    @csrf
                    @method('DELETE')
                    <button type="submit" aria-label="Delete {{ $note->title }}">Delete</button>
                </form>
            </li>
        @empty
            <li>No notes yet. Add the first one above.</li>
        @endforelse
    </ul>
    <small>SQLite is opened by PHP in this application process. No database server or Orbit connection is needed.</small>
</main>
</body>
</html>
