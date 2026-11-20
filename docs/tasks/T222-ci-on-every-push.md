# T222. CI on every push — Open

- **Why:** public-release plan step P3 (`docs/tasks/plans/public-release.md`). The repository is
  public since 2026-11-19, so GitHub's runners cost nothing; the workflow from T220
  (`.github/workflows/ci.yml`) runs today only by hand and on pull requests to `main`. The owner
  decided on 2026-11-20 (PR-Q12) that a push to `main` or to `feature/gui-workspace` also runs it.
  A run per push saves the vcpkg binary cache under a new key every time (item 3), which would
  fill the repository's cache with copies of the same packages, so that is changed with it.
  GitHub keeps caches per branch: a run on `feature/gui-workspace` restores its own and `main`'s,
  a run on `main` only `main`'s, a pull request run its own and `main`'s. No run has saved a cache
  for `main` yet, so the first push run on each branch builds LibRaw and the whole `target` from
  scratch; after that, pull request runs reuse `main`'s caches.
- **Crates:** none. **Files:** `.github/workflows/ci.yml`, `docs/building.md`, `docs/TESTING.md`.
- **What:**
  1. **Trigger:** under `on:`, add `push` with `branches:` `main` and `feature/gui-workspace`
     (both named, no wildcard). Keep `workflow_dispatch` and `pull_request` to `main` as they are.
     Nothing else in `on:`: no `paths` or `paths-ignore` filter, no tags, no schedule. Keep
     `permissions: contents: read`.
  2. **Concurrency group:** today it is `${{ github.workflow }}-${{ github.head_ref || github.ref }}`,
     so two pull requests from different forks whose branches have the same name (`main`,
     `patch-1`) cancel each other. Change it to
     `${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}`, and keep
     `cancel-in-progress: true`. A new push to a branch then cancels that branch's run in progress;
     `main` and the work branch have different groups, so a push of both at the same commit runs
     twice, which PR-Q12 accepts.
  3. **The vcpkg binary cache saved only when the install added a package:**
     - In the step `Install the native libraries with vcpkg`, give it `id: vcpkg-install`. After
       the existing `New-Item -Force` (so the folder exists also when the restore found nothing;
       the step's PowerShell stops on a missing folder), count
       `@(Get-ChildItem -LiteralPath $env:VCPKG_BINARY_CACHE -Recurse -File -Filter *.zip).Count`
       (the `files` provider keeps `<hash>.zip` files in two-letter subfolders; `@()` keeps
       `.Count` right for zero or one file). Count again after the install, after its
       `$LASTEXITCODE` check.
     - Write the literal line `added=true` if the second count is larger, else `added=false`, with
       `Add-Content -Path $env:GITHUB_OUTPUT -Value ... -Encoding ascii` (in Windows PowerShell 5.1
       `>>` writes UTF-16 and `Out-File -Encoding utf8` a byte-order mark; both break the file).
     - The step `Save the vcpkg binary cache` gets `if: steps.vcpkg-install.outputs.added == 'true'`.
       Its key and the restore step (key and `restore-keys`) stay as they are. A run whose restore
       found nothing counts 0 before the install and always saves.
     - The comment above the restore step: its words that the folder is "saved again after every
       install" become that it is saved, under a new key (`run_id`), only when the install added a
       package; the rest stays.
     - Known and accepted: vcpkg never deletes from a `files` cache, so zips for an old compiler
       stay until GitHub evicts the entry; a save that meets an existing key (a re-run) only
       warns.
  4. **The header comment** at the top of `ci.yml` says when the workflow runs: by hand, on a
     push to `main` or `feature/gui-workspace`, and on a pull request to `main`.
  5. **`docs/building.md` §4:** the sentence that names the workflow adds when it runs (the same
     three cases). **`docs/TESTING.md` §11:** the words "started by hand or by a pull request to
     `main`" become the same three cases. §11 asks that CI documentation describe a workflow that
     "has been verified"; that is accepted here, and the reviewer notes in this log that the first
     push run verified it. No other change to either file.
- **Not in this task:** running the workflow or pushing (the reviewer and the owner, after the
  review: the first push after the commit is the test); a status badge in the README; pruning old
  zips from the vcpkg cache; changes to the Cargo cache (it saves only on a `Cargo.lock` change,
  `target` included, per branch; if the repository's 10 GB cache limit then evicts the vcpkg
  cache, that is a follow-up for the backlog), the cargo-audit cache, ExifTool, the toolchain,
  `tools/check.ps1` or any build file; branch rulesets or other GitHub settings; the real-file
  tests in CI (plan R9); Linux or macOS jobs.
- **Rules:** the rules of T220 still hold: only GitHub's own actions, pinned by full commit SHA
  with the version in a comment (no new action is needed here; reuse the pinned ones); no secrets;
  no personal data and no literal path into a user folder (`check.ps1`'s `personal data` step
  reads the workflow); nothing downloaded or cached inside the checkout. PowerShell 5.1 is the
  step shell (`defaults.run.shell: powershell`): no `&&`, `||`, `?:` or `??`. Do not change the
  meaning of an existing step beyond items 1–4.
- **Done when:** `.\tools\check.ps1` is all OK; `git diff` shows, besides the changes that were in
  the worktree when the task started, only the three files above, and this file only its status
  and one Log row. The report says that the save condition is "the number of `*.zip` files under
  the cache folder went up", and that `ci.yml` was checked only by reading (`check.ps1` does not
  validate YAML; the workflow runs on GitHub only).

## Log

| Date | Status | Note |
|---|---|---|
| 2026-11-20 | Open | Added (public-release plan P3, PR-Q12). Cold read (fresh agent, 2026-11-20): can be done with the named files; fixed in the text: the cache count recursive and on `*.zip` only, after `New-Item`, with the step id and output named and written as ASCII; the cache comment's "after every install" changed instead of kept; the concurrency group by pull request number (forks' same-named branches cancelled each other); the cache scope per branch and the first runs from scratch in Why; the Cargo cache's size and the vcpkg cache's growth named as accepted; §11's "verified" rule met by the reviewer's log row after the first push; Done when by `git diff`, and the YAML checked only by reading |
