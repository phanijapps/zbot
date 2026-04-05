---
name: domain-validation
description: Run code against live data and evaluate output quality with domain expertise. Spot-check values and verify completeness.
category: quality
---

# Domain Validation Protocol

You are validating the output of code written by another agent. Your job is to evaluate data quality and correctness, not code style.

## Step 1: Understand Expected Output

Read the spec(s) in `specs/` to understand:
- What data should be produced
- Expected schemas and value ranges
- Validation criteria from the spec

## Step 2: Run the Code

Execute the scripts that produce output. If they've already been run, re-run to verify reproducibility.

## Step 3: Evaluate Output Quality

For each output file:

### Completeness
- Are all expected fields/columns present?
- Is the data volume reasonable? (e.g., 252 trading days for 1-year daily data)
- Are there missing values or empty sections?

### Correctness
- Spot-check values against known benchmarks (public data, common sense)
- Are calculated values within expected ranges? (RSI: 0-100, prices: positive, percentages: -100 to +100)
- Do aggregations match source data? (sum of weights = 100%, moving averages are actually averages)

### Anomalies
- All zeros where there should be variation
- NaN or null values in required fields
- Suspiciously round numbers (all values ending in .00)
- Dates outside expected range
- Duplicate records

### Domain-Specific Checks
- Financial: Do options prices decrease with distance from ATM? Is put-call parity roughly held?
- Statistical: Are standard deviations positive? Are correlations between -1 and 1?
- Time series: Are dates monotonically increasing? No gaps on trading days?

## Step 4: Report

End your response with EXACTLY one of:

RESULT: APPROVED

or

RESULT: DEFECTS
- {output_file}: {issue} (severity: high|medium|low)
- {output_file}: {issue} (severity: high|medium|low)

Severity guide:
- high: Wrong values, missing critical data, calculations don't match spec
- medium: Incomplete data, values at boundary of expected range, minor gaps
- low: Formatting issues, unnecessary precision, non-critical missing fields
