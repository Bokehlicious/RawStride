# T282. A code of conduct and a pull request template (docs, `.github/`) — Done

- **Why:** GitHub's Community Standards page (Insights → Community Standards) marks two items as
  missing: a code of conduct and a pull request template. README, LICENSE, CONTRIBUTING,
  `.github/SECURITY.md` and the issue templates are already there (public-release plan R8, PR-Q8).
  `CONTRIBUTING.md` welcomes pull requests from outside, so a template that asks for what
  CONTRIBUTING already requires saves the reviewer a round of questions, and a code of conduct
  says how people are expected to behave in issues, Discussions and pull requests.
- **Files:** `CODE_OF_CONDUCT.md` (new, repository root), `.github/pull_request_template.md`
  (new), `CONTRIBUTING.md` (one line). No code, no change to `tools/check.ps1`.
- **Owner decisions (owner, 2026-11-22: every proposal approved; Q2 is (a)):**

  | Id | Question | Proposal (approved) |
  |---|---|---|
  | Q1 | **Which code of conduct text?** | The Contributor Covenant, version 2.1, unchanged except the contact. It is the text GitHub recognises and offers itself; it is licensed CC BY 4.0, and its attribution section stays in the file. |
  | Q2 | **How is a breach reported?** The text needs a contact. `check.ps1`'s personal-data step fails on any real e-mail address outside its allowed domains, and the owner's own address is not to be published (PR-Q2). Options: (a) no e-mail: report privately to the maintainer through GitHub's private vulnerability reporting, as `SECURITY.md` describes, with "Code of conduct" in the title; (b) a new address only for the project, and `CODE_OF_CONDUCT.md` skipped for e-mail addresses in `check.ps1` the way `THIRD-PARTY-NOTICES.txt` is (T227); (c) as (b), but that one address allowed exactly instead of the whole file skipped. | (a): nothing new to publish or watch, and no change to `check.ps1`. Its drawback: the form is made for security reports, so the text must say plainly that it is used for this too. |
  | Q3 | **What goes in the pull request template?** | Short, and points to `CONTRIBUTING.md` rather than repeating it: what the change does and why; the issue it belongs to (required for a larger change, CONTRIBUTING "Before you start"); a check list: `.\tools\check.ps1` all OK, which tests were run and whether `-Ignored` was; every commit carries `Signed-off-by`; no new dependency without an issue first; nothing derived from another product (clean room). In English, as all repository text. |

- **What:**
  1. `CODE_OF_CONDUCT.md` at the root: the Contributor Covenant 2.1 (Q1), copied word for word
     from its canonical text,
     <https://www.contributor-covenant.org/version/2/1/code_of_conduct/> (the Markdown file
     `code_of_conduct.md` in the project's repository); fetch it, do not write it from memory. Its
     only placeholder, `[INSERT CONTACT METHOD]` in "Enforcement", is replaced by Q2 (a): report
     privately to the maintainer through the repository's **Security** tab, **Report a
     vulnerability**, with "Code of conduct" at the start of the title. Say plainly that this form is
     used for conduct reports too; that a report is seen only by the reporter and the maintainer (it
     opens a draft security advisory); and that, unlike a security report under `SECURITY.md`, a
     conduct report is never published and `SECURITY.md`'s steps (fix, release, publish) do not apply
     to it. Everything else ("community leaders" and the rest) stays as it is: no names, no e-mail
     address.
  2. `.github/pull_request_template.md` as decided in Q3, in Markdown with HTML comments for the
     hints (they do not show in the opened pull request), and `- [ ]` check boxes. Besides Q3's list:
     a box "A change in behaviour comes with a test" (CONTRIBUTING, "Rules for the code"), and a hint
     comment that commits are kept as they are once the review starts, no force push (CONTRIBUTING,
     "Pull requests").
  3. `CONTRIBUTING.md`: one sentence, as a new paragraph after the opening paragraph and before
     "## Before you start", that everyone taking part follows the
     [code of conduct](CODE_OF_CONDUCT.md). Nothing else in it changes.
- **Tests to add:** none (documents only).
- **Not in this task:** issue template changes (a conduct link in `ISSUE_TEMPLATE/config.yml`);
  a link from `README.md`; a `CODEOWNERS` file; a `FUNDING.yml`; any GitHub setting (the owner's,
  on the site; private vulnerability reporting is already on, public-release plan P2b,
  2026-11-19).
- **Rules:** `AGENTS.md`; no personal data (no user-folder path, no real e-mail address). Docs
  only: `check.ps1` is not run, but the staged diff is searched for user-folder paths and e-mail
  addresses before the commit.
- **Done when:** the files above exist as decided, `git diff --stat` shows only them, and after the
  push GitHub's Community Standards page shows both items as done (the owner looks). GitHub reads
  both files only from the default branch, so the page is checked after `main` has been
  fast-forwarded and pushed (the push routine, on the owner's yes).

## Log

| Date | Who | What |
|---|---|---|
| 2026-11-22 | reviewer | Written at the owner's request after GitHub's Community Standards page showed the two items missing. Blocked on Q1–Q3. Not yet in the index (`README.md` "Open tasks"): the row is added when T279's agent has finished, so the two do not edit the index at the same time. |
| 2026-11-22 | owner | Q1–Q3: every proposal approved; Q2 is (a), so no e-mail address and no change to `check.ps1`. |
| 2026-11-22 | reviewer | Cold read: the Covenant's source URL and its one placeholder named; the conduct report never published and seen by the reporter and the maintainer (`SECURITY.md` publishes security reports); private reporting already on (P2b); the template's list gained the test box and the no-force-push hint; the CONTRIBUTING sentence placed; "Done when" checked after `main` is pushed. A README link and a conduct link in the issue chooser left out (not asked for). Set to Open; the index row and "T283 next" added in the same commit (the index was untouched by T279's agent). |
| 2026-11-22 | task agent | Ready for review. `CODE_OF_CONDUCT.md`: the Contributor Covenant 2.1 from the project's repository (`EthicalSource/contributor_covenant`, `content/version/2/1/code_of_conduct.md`, the site's own Markdown copy compared: only blank lines differ), its Hugo front matter left out, `[INSERT CONTACT METHOD]` replaced by Q2 (a) (Security tab, Report a vulnerability, "Code of conduct" at the start of the title; also used for conduct reports; seen only by the reporter and the maintainer; never published, SECURITY.md's steps do not apply); nothing else changed. `.github/pull_request_template.md`: Q3's list, the test box and the no-force-push hint. `CONTRIBUTING.md`: one paragraph after the opening one. Docs only, check.ps1 not run. |
| 2026-11-22 | reviewer | Done. Reviewed: `CODE_OF_CONDUCT.md` compared with the Covenant's canonical Markdown (`EthicalSource/contributor_covenant`, `content/version/2/1/code_of_conduct.md`): apart from the front matter and blank lines, only the contact sentence differs, and it says everything point 1 asks; the template has Q3's list, the test box and the no-force-push hint as HTML comments; CONTRIBUTING has the one paragraph. LF line endings; the files searched for user-folder paths and e-mail addresses (only CONTRIBUTING's existing `example.com`). Docs only, check.ps1 not run. Left: the owner looks at GitHub's Community Standards page after `main` is pushed |
| 2026-11-22 | reviewer | Checked after `main` was pushed (1dce073), with `gh api repos/…/community/profile`: health 100 %, `code_of_conduct` and `pull_request_template` found. "Done when" met |
