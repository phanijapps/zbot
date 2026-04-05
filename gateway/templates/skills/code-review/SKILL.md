---
name: code-review
description: Review code against specs for quality, correctness, modularity, and adherence to ward conventions.
category: quality
---

# Code Review Protocol

You are reviewing code written by another agent. Be thorough and critical.

## Step 1: Read the Spec

Find the spec(s) in `specs/` that correspond to the code being reviewed.
Check all 8 mandatory sections: Purpose, Inputs, Outputs, Algorithm, Dependencies, Error handling, Validation, Core module candidates.

## Step 2: Read the Code

Read every file the coding agent created or modified.
For each file, check:
- Does it match the spec's algorithm and data flow?
- Does it use core/ modules where they exist? (check AGENTS.md for available modules)
- Is it under 100 lines?
- Is error handling present for API calls, file I/O, missing data?
- Are there hardcoded values that should be parameters?

## Step 3: Run the Code

Execute the scripts and verify:
- No runtime errors
- Output files are created at the paths specified in the spec
- Output format matches the spec's schema

## Step 4: Verify Output Structure

Read the output files. Check:
- JSON files parse correctly and have all expected keys
- CSV files have expected columns and reasonable row counts
- Values are within expected ranges (no NaN, no zeros where there shouldn't be)

## Step 5: Report

End your response with EXACTLY one of:

RESULT: APPROVED

or

RESULT: DEFECTS
- {file}: {issue} (severity: high|medium|low)
- {file}: {issue} (severity: high|medium|low)

Severity guide:
- high: Wrong algorithm, missing functionality, runtime errors, data loss
- medium: Missing error handling, hardcoded values, no validation
- low: Style issues, could be more modular, minor optimization
