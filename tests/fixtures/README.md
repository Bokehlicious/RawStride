# Test fixtures

Only small files that we are allowed to redistribute may go here.

The full RAW test sets (Sony A1 / A7R IVA) live outside the repo, in
`D:\PhotoWorkflowData\RAW` and `D:\PhotoWorkflowData\TestImages`.
Tests that need them read the path from the `PW_TESTDATA` environment variable
and are skipped when it is not set.
