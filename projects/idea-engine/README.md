# Idea Evolution Engine

A terminal app (a VBR `Screen`) that asks an LLM for scored project ideas,
stores them in SQLite, and shows the current best. It's the running example that
drove much of VBR's stdlib forward — `Http.Post`, `Database`, the `Json`
builder, list literals, `Chr`/`vbNewLine`.

## What it does (slice 1)

Press **`g`** to generate a batch: the app sends your challenge + context to the
LLM, asks for 5 scored ideas as JSON, stores them in `ideas.db`, and shows the
top ideas ranked by score. Press **`q`** to quit. Each press is one generation.

Still to come (later slices): editing challenge/context in-app, mutating and
merging ideas across generations, and inspecting lineage.

## Setup

1. Put your LLM details in `config.json` (an OpenAI-compatible chat-completions
   endpoint):

   ```json
   {
     "endpoint": "https://api.openai.com/v1/chat/completions",
     "api_key": "sk-...",
     "model": "gpt-4o-mini",
     "challenge": "Create a useful, monetisable software project",
     "context": "...your constraints and preferences for judging ideas..."
   }
   ```

2. Build and run:

   ```sh
   cargo run -- build projects/idea-engine
   cd projects/idea-engine/build
   cp ../config.json .        # the app reads config.json from where it runs
   cargo run
   ```

   > **Note:** the program reads `config.json` (and writes `ideas.db`) in the
   > *current working directory*, which for a project build is `build/`. Copy
   > your `config.json` there, or run the built binary from a folder that has
   > one. (VBR projects don't yet copy data files into `build/` — logged in
   > `../vbr_gaps.md`.)

If `config.json` is missing or malformed, or the database can't be opened, the
app stops at startup with `could not start: <why>` — it never launches with
broken state (VBR's fallible `State` initialisers).

## How it's built

One file, `main.vbr`:

- **State** holds the open `Database` and parsed `Json` config — both *fallible*
  initialisers, so a bad setup fails cleanly before the UI starts.
- **`Generate`** builds the chat request with the `Json` builder (so the prompt
  is escaped correctly), `Await`s `Http.Post` off the UI thread, then in the
  continuation parses the reply, stores each idea, and re-queries the ranked
  best for display.
- Helper functions do the config reading, prompt building, and DB work — the
  event body stays readable.
