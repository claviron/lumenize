# Lumenize contributor and agent guidance

These instructions are public and apply to contributors and automated agents.
Keep them portable and useful without access to the maintainers' machines.

## Read before changing code

Read `README.md` and `ARCHITECTURE.md` first. Read a component's own instructions
before changing it. If `AGENTS.local.md` exists, read it for private checkout
context; public builds and documentation must not require that file.

## Architecture

- Lumenize is a Laravel-focused PHP runner built on Claviron. The downstream
  application owns its binary, entry point, configuration, and business logic.
- Use Claviron's public facade. Do not depend on its internal crate paths or
  duplicate its PHP engine, supervision, or shared-state lifecycle.
- Keep common Laravel integration usable without Lumenize. Runner-specific
  composition and starter tooling belong here.
- Register definitions without resolving services during registration.
  Resolve dependencies during boot. Long-running runtime services implement
  `Runnable` and cooperate with shutdown for the work they own.
- Make optional capabilities explicit. Do not silently enable edge services
  or additional language runtimes for every application.
- Do not invent timeouts, polling intervals, or termination policies.
  Use the owning configuration or API contract; ask when no policy exists.

## Changes and verification

- Preserve unrelated working-tree changes and keep edits scoped to the task.
- Update architecture documentation when ownership or lifecycle contracts change.
- Distinguish implemented behavior from planned capabilities in public docs.
- Run focused checks for the affected package. For the current library skeleton,
  `cargo check -p lumenize` is sufficient to verify compilation.
- Commit messages describe the change, without assistant attribution.
- Publishing, pushing, and opening pull requests require explicit authorization.

## Public and local documentation

- `AGENTS.md` contains only public, reusable project guidance. It may ship to
  end users along with the crate.
- `AGENTS.local.md` contains private checkout context, machine paths,
  internal project comparisons, and maintainer-specific notes. Do not commit,
  publish, or copy it into public documentation or generated projects.
- Never put credentials or other secrets in either file.
- Keep the Cargo package allowlist explicit and verify it before release.
