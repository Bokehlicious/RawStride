# Test fixtures

Only small files that we are allowed to redistribute may go here.

The full RAW test sets (Sony A1 / A7R IVA) live outside the repo (the maintainer keeps them in
`D:\PhotoWorkflowData\RAW` and `D:\PhotoWorkflowData\TestImages`).
Tests that need them read the path from the `PW_TESTDATA` environment variable
and are ignored by default. Set `PW_TESTDATA` to the appropriate folder and run
`$env:PW_TESTDATA = "<folder>"; cargo test -p <crate> -- --ignored` to execute them.
