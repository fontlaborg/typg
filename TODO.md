# typg — TODO
made by FontLab https://www.fontlab.com/

Outstanding modernization work. Completed items move to `CHANGELOG.md`.
The `--details` / `--csv` feature plan that previously lived here shipped in full;
its history now lives in `CHANGELOG.md`.

## Documentation
- [ ] Add `src_docs/` (MaterialX / MkDocs-Material) compiling into `docs/`: architecture overview (data flow 1→7 from CLAUDE.md), `Query` builder API reference, environment-variables table.
- [ ] Add `QUICKSTART.md` covering the "install → find → cache" workflow in five commands.
- [ ] Split the README "CLI usage" detail into `docs/cli.md` once the CLI surface grows past what keeps the README under 200 lines (currently 177).
- [ ] Verify `docs/spec.md` lists the OS/2 weight/width/family-class filters and the `hpindex` divergence from fontgrep.

## Tests / CI
- [ ] Add a root `test.sh` that runs `cargo test --workspace && pytest py/typg-python/tests` so test discovery is trivial.
- [ ] Add an integration test that boots the Axum server (`server.rs`) and hits `GET /health` and `POST /search`.
- [ ] Add a `proptest` covering `--creator` / `--license` regex with special characters (e.g. parentheses in copyright strings).

## Types / API ergonomics
- [ ] Ship a `.pyi` type stub for `find()`, `find_paths()`, `find_indexed()` so IDE completion works for Python callers.
- [ ] Document that the native extension `typg_python._typg_python` is an internal detail; verify the wheel import chain after `maturin develop`.

## Housekeeping (post-release)
- [ ] Untrack committed agent/orchestration state and build cruft now that `.gitignore` covers it: `git rm -r --cached .omc .codegraph .swarm llms.txt py/typg-python/python/typg_python/*.dSYM` in a dedicated commit.
