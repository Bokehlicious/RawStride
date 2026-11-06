# Testing Strategy for PhotoWorkflow

## Introduction

This document describes the testing strategy for the PhotoWorkflow project. It defines principles for golden-output verification, test tiers, and when different types of tests should be used.

**Note**: Test implementation is not performed during this task. This is documentation of the strategy to be implemented in future CI workflows.

## 1. Golden-Output Principles

Golden output defines reference outputs against which new builds can be compared.

### 1.1 Metadata Handling

#### Semantic Verification

- Validate field structure and value semantics (e.g., IPTC field names, XMP schema).
- Ensure values have meaningful context-appropriate meanings.

#### Round-Trip Verification

- Test that metadata survives a full pipeline (read -> process -> write).

### 1.2 Deterministic CPU Operations

- Use reproducible tests for preprocessor code and binary calculations.
- Perform byte-exact comparisons where appropriate (mathematical constants, hash values).
- Avoid randomization or timestamp-dependent values in golden output.

### 1.3 RAW Decoding

#### Verified References

- Known file formats have verified reference outputs (e.g., Adobe DNG specification).
- Compare decoding results against known references when available.

#### Environment Variations

- Document actual variations between build environments as allowed.
- Long-term environment dependencies are documented in docs/analyses/.

### 1.4 GPU Rendering

#### Tolerance-Based Comparison

- Pixel values may differ between builds due to rendering order, floating-point precision.
- Tolerances should be determined empirically via benchmarking against reference builds.
- Perceptual comparison is used (LPIPS/DSSIM as optional tools, not mandatory).

#### Rendering-Varying Factors

- GPU rendering tolerates certain differences (shader execution order, floating-point round-off).
- Avoid byte-exact comparisons when rendering is involved.

## 2. Test Tiers and Usage Cases

Tests are organized into four tiers:

### 2.1 Fast Gate

**Purpose**: Basic functional tests that should always succeed.
**When used**: All branch merges, release builds.
**Includes**:
- Basic compilation checks
- API compatibility (when applicable)
- Metadata round-trip for known formats

### 2.2 Targeted Tests

**Purpose**: Tests for specific functionality areas or problem domains.
**When used**: Local changes within a domain (e.g., RAW decoding, GPU rendering).
**Includes**:
- Format-specific tests (RAW, JPEG, TIFF)
- Feature tests for new functionality
- Regression tests for changed areas

### 2.3 Comprehensive Tests

**Purpose**: Full-system tests including extensive performance and memory checks.
**When used**: Merge to main branch when risk/size is high.
**Criteria for when comprehensive tests are required**:
1. Change risk and scope justify it (e.g., core code, renderer).
2. New dependencies are added or changed.
3. Heritage between build environments is affected (OpenMP, GPU rendering).

### 2.4 Release Controls

**Purpose**: Performance, memory, and compatibility controls prior to release.
**When used**: New versions, packaging.
**Includes**:
- Memory leak detection (optional tooling)
- Performance benchmarking
- Cross-platform compatibility (when applicable)

## 3. Comparison Strategies

The following comparison methods are used depending on data and context:

| Strategy | Usage | Example |
|----------|-------|---------|
| Byte-exact | Metadata, deterministic calculations | Hash values, numerical constants |
| Tolerance-based | GPU rendering, preprocessor results | Pixel difference (<1 pixel), shader output |
| Perceptual | Rendering comparisons | LPIPS/DSSIM as optional tools |

### 3.1 Byte-Exact Comparison

- Metadata hash values (MD5/SHA).
- Known numerical constants (pi, e, etc.).
- File formats with exact specifications (e.g., JPEG quality 100%).

### 3.2 Tolerance-Based Comparison

- GPU rendering (max diff < 1 pixel).
- Preprocessor results where environment-sensitive (OpenMP parallelization).
- Timestamp-dependent values are avoided.

### 3.3 Perceptual Comparison

- Rendering comparisons (LPIPS/DSSIM as optional tools).
- User-driven test cases (manual review when automatic detection is unclear).

**Important**: LPIPS, SSIM, and DSSIM are optional tools, not mandatory dependencies. Threshold values should be determined empirically via benchmarking.

Avoid byte-exact comparison when environment variations are expected (e.g., timestamping, UUIDs).

## 4. CI Strategy (Documentation)

### 4.1 Test Tier Descriptions

The following test tiers should be implemented when CI workflows are created:

| Workflow | Purpose | Required For |
|----------|---------|--------------|
| fast-gate | Basic functional tests, fast feedback | All merge requests |
| targeted-tests | Tests specific to changed areas | Local updates, small changes |
| comprehensive-tests | Full-system test including extensive performance/memory checks | Merge to main with justified risk/scope |
| release-controls | Performance-memory-compatibility controls for release | Release branches, packaging |

### 4.2 When Comprehensive Tests Are Required

Comprehensive tests should be required before merge when:
1. Change risk and scope justify it (e.g., core code, renderer).
2. New dependencies are added or changed.
3. Heritage between build environments is affected (OpenMP, GPU rendering).

### 4.3 Documentation Status

- .github/workflows/ci.yml should not be created during this task.
- The testing strategy is documented here; implementation occurs in future project phases.

---

## 5. Analysis Files

Analysis files for specific investigations are stored in docs/analyses/:

| File | Purpose |
|------|---------|
| raw-decoding-analysis.md | Investigation of RAW decoding variations between build environments |
| iptc-metadata-validation.md | Detailed analysis of IPTC metadata needs and round-trip verification |
| adr0008-decision-matrix.md | Decision matrix for ADR0008 (e.g., rendering pipeline decisions) |

## 6. References

- AGENTS.md#golden-output-and-test-strategy - Shared test principles
- docs/clean-room.md - Clean-room policy for legal scoping
- docs/adr/ - Architecture decisions and technical choices

---

*This document describes the testing strategy. Implementation is deferred to future CI workflow creation.*
