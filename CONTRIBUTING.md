# Contributing to RawStride

Thank you for your interest. RawStride is a small project with one maintainer, so a little
coordination up front saves everyone time.

## Before you start

- **Bugs and ideas:** open an issue. For a bug, say what you did, what you expected, what happened,
  the camera and file type, and the RawStride commit or version.
- **Small fixes** (a typo, a clear bug with an obvious fix): a pull request is welcome directly.
- **Larger changes** (a new feature, a new dependency, a change of behaviour or of the file format
  RawStride writes): open an issue first and wait for the maintainer's answer. Many decisions are
  already recorded in the ADRs under `docs/adr/`; a change that goes against one needs that
  discussion first.

## Building and checking

Follow [docs/building.md](docs/building.md). Before you open a pull request, run

```powershell
.\tools\check.ps1
```

and make sure every step (format, tests, clippy) is OK. Say in the pull request which tests you
ran, and whether you ran the ignored real-file tests (`-Ignored`).

## Rules for the code

- **Never crash:** no `.unwrap()`, `.expect()`, `panic!`, `unreachable!` or unchecked indexing
  outside tests. Return errors instead.
- **No warnings:** `cargo clippy` must be clean; format with `cargo fmt --all`.
- **No new dependencies** without the maintainer's agreement in an issue first. A dependency's
  licence must be compatible with GPL-3.0-or-later.
- **Texts the user sees** are English, use the terms of [docs/glossary.md](docs/glossary.md), and
  are made by a function in a text module, one whole sentence from one template
  ([docs/tasks/README.md](docs/tasks/README.md), rule 13).
- **Tests:** a change in behaviour comes with a test. Never weaken or remove a test to make a change
  pass. [docs/TESTING.md](docs/TESTING.md) explains the tests.
- **Metadata:** RawStride follows the IPTC standards ([docs/references.md](docs/references.md)) and
  never modifies a proprietary RAW file; see ADR 0007 before changing anything that writes.
- Code, comments, documentation and commit messages are in English.

## Clean room

RawStride is developed under a [clean-room policy](docs/clean-room.md). Do not contribute anything
derived from another product's code, binaries, assets or decompiled material, and do not contribute
code you are not free to license under GPL-3.0-or-later.

## Licence and sign-off

RawStride is licensed under the GNU General Public License, version 3 or later
([LICENSE](LICENSE)). By contributing, you agree that your contribution is licensed under the same
terms.

Every commit must carry a `Signed-off-by` line, which certifies the
[Developer Certificate of Origin](https://developercertificate.org): that you wrote the change or
otherwise have the right to submit it under the project's licence. `git commit -s` adds the line:

```
Signed-off-by: Your Name <you@example.com>
```

## How the project itself works

The maintainer works with coding agents. Their rules are in [AGENTS.md](AGENTS.md), and their tasks
are in [docs/tasks/](docs/tasks/README.md). You do not need to follow that workflow, but the tasks
show what is planned, and the backlog ([docs/tasks/backlog.md](docs/tasks/backlog.md)) what is
waiting for a decision.
