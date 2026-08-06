# Contributing

Thanks for considering a contribution to Harmonia! This project aims to be a
production-quality music player, so every contribution is held to a high bar —
in the best way.

## Code of conduct

Be respectful. We review code, not people. Harassment of any kind is not
tolerated.

## What to work on

- Check the [Roadmap](ROADMAP.md) for planned work.
- Look for `help wanted`-style gaps: missing tests, untranslated strings,
  missing docs, performance hot spots.
- **File an issue first** for anything bigger than a bugfix so we can agree on
  the approach before you spend hours on it.

## Getting started

1. Fork and clone the repo.
2. Follow [DEVELOPMENT.md](DEVELOPMENT.md) to set up your environment.
3. Create a branch: `git checkout -b feat/my-change`.

## Code style

- **Rust**: run `cargo fmt` and `cargo clippy -- -D warnings`. Match the
  existing structure (core logic in `harmonia-core`, shell concerns in
  `src-tauri`).
- **TypeScript**: strict mode is already on; keep it that way. No `any`
  escapes unless justified with a comment.
- **CSS**: use the design tokens in `src/styles/global.css`. Prefer CSS custom
  properties over literals.
- **i18n**: all UI strings via `t("key")`. When adding a key, add it to both
  `en.json` and `es.json`.
- **No placeholders** — no `TODO`, `unimplemented!()`, or mock code.

## Testing your change

```bash
cargo test -p harmonia-core
cargo clippy -p harmonia-core -- -D warnings
cargo fmt -p harmonia-core --check
npx tsc --noEmit
npm run build
```

If you touch the audio engine or scanner, add unit tests for the core logic.

## Submitting a pull request

1. Keep changes focused — one logical change per PR.
2. Write a clear PR description: what and why.
3. Make sure CI is green (tests, clippy, frontend build, packaging).
4. Respond to review comments; rebase when requested.

## Review checklist

- [ ] No panics on malformed user data
- [ ] New strings localized (en + es)
- [ ] No dead code / unused imports
- [ ] Tests cover new core logic
- [ ] clippy + fmt clean

## License

By contributing you agree that your contributions are licensed under the MIT
License, as described in [LICENSE](../LICENSE).
