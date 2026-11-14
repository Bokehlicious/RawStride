# Testing Strategy — Mandatory Requirements

## 1. Purpose

This document defines how RawStride is tested, how test results are evaluated, and how implementation correctness is verified against approved project requirements.

The testing strategy must:
- Verify observable product behavior rather than merely implementation details.
- Connect tests to requirements and ADR decisions.
- Distinguish automated tests from manual verification.
- Distinguish simulated tests from tests using real files and real application components.
- Make skipped and unavailable tests visible.
- Avoid inventing acceptance criteria that have not been approved.

## 2. Source of Truth

Testing must be based on:
- Explicit project-owner decisions.
- Approved product requirements.
- Accepted ADRs and their accepted amendments.
- Approved implementation specifications.

A proposed ADR is not an approved implementation contract.

Prototype measurements and experimental results must not automatically become production acceptance criteria.

When a requirement is unresolved, tests may document the intended verification area, but must not claim that the final acceptance criteria are established.

## 3. Metadata Storage and Integrity

The following storage policy is fixed:

| File format | Metadata storage | Original image modification |
|---|---|---|
| DNG | Embedded in DNG | Allowed and required for metadata writes |
| Other RAW formats, including ARW and RW2 | Adjacent XMP sidecar | Original RAW must not be modified |
| JPEG | Embedded in JPEG | Allowed and required for metadata writes |
| TIFF | Embedded in TIFF | Allowed and required for metadata writes |

Tests must respect these rules.

Full-file hash equality must not be expected after an intentional embedded metadata write to DNG, JPEG, or TIFF.

For RAW formats using sidecars, tests must verify that metadata operations leave the original RAW file unchanged.

Tests should verify:
- Correct metadata storage location.
- Preservation of unrelated metadata.
- Preservation of existing sidecar content where required.
- Correct behavior when sidecars are missing or already exist.
- Correct handling of Unicode and special characters in paths.
- Correct handling of metadata read, write, update, and clear operations.
- Image data integrity where embedded metadata is modified.

Detailed IPTC/XMP behavior must follow the approved metadata ADR and subsequent project-owner decisions.

## 4. Test Categories

### 4.1 Unit Tests

Test individual functions and components in isolation.

Examples:
- Filename and path handling.
- Metadata mapping.
- Filtering and sorting.
- Geometry and coordinate calculations.
- Cache key generation.
- Scheduling and prioritization logic.

### 4.2 Rust Integration Tests

Test interactions between Rust crates and external components.

Examples:
- Ingest planning and execution.
- RAW decoding.
- Metadata operations.
- Rendering engine behavior.
- File integrity and error handling.

### 4.3 UI Regression Tests

Use the existing Node.js and Playwright tests where applicable.

The current UI tests use a mocked Tauri backend.

They verify frontend behavior but do not, by themselves, verify the complete integration with the real Rust backend or WebView2 runtime.

### 4.4 Real-File Tests

Use real camera files when behavior depends on actual RAW formats, embedded previews, metadata, orientation, or decoder behavior.

The external test dataset is configured through `PW_TESTDATA`.

Tests that require this dataset must be clearly reported as skipped or unavailable when it is missing.

### 4.5 External Tool Tests

Tests requiring ExifTool must verify that ExifTool is available.

A test that returns early because ExifTool is unavailable must not be reported as a successful verification.

The configured executable may be selected through `PW_EXIFTOOL_PL` where supported.

### 4.6 Performance Tests

Performance testing must measure observable behavior and use documented test conditions.

Every reported measurement must identify:
- Hardware.
- Operating system.
- Build configuration.
- Test dataset.
- Measurement method.
- Number of runs or sampling method.
- Result and relevant distribution statistics.

Do not convert measurements from a prototype, one machine, or one test run into universal acceptance criteria without explicit approval.

### 4.7 Manual Acceptance Tests

Some requirements require verification in the running desktop application.

Manual tests must document:
- Preconditions.
- Exact test procedure.
- Expected result.
- Actual result.
- Pass/fail status.
- Relevant screenshots, logs, or measurements.

## 5. Test Execution Profiles

The testing strategy should define separate execution profiles.

### Fast Regression

Purpose:
- Quickly detect common regressions.
- Run suitable unit and integration tests.
- Avoid requiring large external datasets where possible.

### UI Regression

Purpose:
- Verify frontend behavior using the existing Playwright tests.
- Report the mocked-backend limitation.

### Metadata Integration

Purpose:
- Verify metadata behavior using ExifTool.
- Test DNG, JPEG, TIFF, and RAW sidecar behavior according to the storage policy.

### Real RAW Verification

Purpose:
- Verify behavior using the external RAW test dataset.
- Cover supported cameras and formats represented in the dataset.

### Rendering and Memory Verification

Purpose:
- Verify rendering engine behavior, scheduling, memory limits, cache behavior, and resource cleanup.

### Full Verification

Purpose:
- Combine all applicable automated tests, real-file tests, performance measurements, and manual acceptance checks.

Exact commands and prerequisites must be verified against the repository before being documented.

Do not invent a unified test runner if none exists.

## 6. Test Result Classification

Every test or test group must be classified as one of:

- **Passed:** The test executed and all assertions succeeded.
- **Failed:** The test executed and one or more assertions failed.
- **Skipped:** The test was intentionally not executed or skipped because a prerequisite was unavailable.
- **Blocked:** The test could not be completed because of an environmental or technical problem.
- **Not run:** The test was not executed.
- **Not applicable:** The test does not apply to the tested configuration.

A skipped test must never be counted as a passed verification.

A test command that exits successfully does not prove that all intended tests actually executed.

## 7. Requirement Traceability

Every important product requirement must be traceable to its verification method.

The traceability matrix should contain:

| Field | Description |
|---|---|
| Requirement ID | Exact identifier from the requirement or ADR |
| Requirement | Short description |
| Source | Requirement document or ADR |
| Verification method | Automated, real-file, performance, or manual |
| Existing test | Exact test file or test name, if present |
| Coverage status | Verified, partially verified, not verified, or blocked |
| Notes | Limitations and outstanding work |

The existence of a related test does not automatically mean that the complete requirement is verified.

## 8. Performance and Acceptance Criteria

Acceptance thresholds must come from approved requirements.

For image viewer requirements V1–V8 and G1–G4:
- Preserve the exact requirement wording and thresholds.
- Do not replace them with more convenient targets.
- Do not claim compliance based only on simulated tests.
- Identify which requirements need measurements in the actual desktop application.
- Record hardware and test conditions for performance results.

Prototype results from ADR 0008 must remain identified as prototype evidence unless explicitly adopted by the project owner.

## 9. Test Data

Small files that may legally be redistributed may be stored in the repository's fixture directory.

Large RAW datasets must remain outside the repository.

The current external test dataset is documented in `tests/fixtures/README.md` and accessed through `PW_TESTDATA`.

Tests must not silently substitute synthetic files for required real-file verification.

Synthetic fixtures and real camera files serve different purposes and must be reported separately.

## 10. Test Integrity

- Do not weaken assertions to make tests pass.
- Do not remove tests merely because they are inconvenient.
- Do not change expected behavior without an approved requirement change.
- Do not report unexecuted tests as successful.
- Do not hide missing external dependencies.
- Do not treat mocked backend tests as full integration tests.
- Do not treat prototype benchmarks as product acceptance results.
- Do not use full-file hashes as an integrity criterion when embedded metadata writes are expected.
- Use disposable copies or temporary files for tests that modify image metadata.

## 11. CI Strategy

CI documentation must describe the actual implemented CI configuration.

Do not claim that tests run on every commit, pull request, branch, or release unless the corresponding workflow exists and has been verified.

Do not create or modify CI workflows as part of a documentation-only task.

## 12. Test Documentation Maintenance

When tests are added, removed, or materially changed:
- Update the relevant test inventory.
- Update requirement traceability where necessary.
- Document new external dependencies.
- Document new environment variables.
- Document new test data requirements.
- Update exact execution commands only after verifying them.

Keep the test strategy aligned with the actual repository.

Do not document planned tests as existing tests.
