---
description: Comprehensive Clean Code Scan and Auto-Fix using SonarQube MCP
---

# SonarQube Clean Code Agent

Use `sonarqube` MCP tools to scan, resolve, and refactor.

## 1. Security Scan
- **Identify**: Run `sonarqube.search_issues` for `VULNERABILITY` and `SECURITY_HOTSPOT`.
- **Action**: Fix flaws or log risks in `sonar_fix_report.md`.

## 2. Reliability Scan
- **Identify**: Run `sonarqube.search_issues` for `BUG`.
- **Action**: Fix logic errors in the source code.

## 3. Maintainability Scan
- **Identify**: Run `sonarqube.search_issues` for `CODE_SMELL`.
- **Action**: Refactor complex logic, remove dead code, technical debt, and improve naming.


## 4. Duplication Scan (DRY)
- **Identify**: Use `sonarqube.get_sonarqube_metrics` for `duplicated_blocks`.
- **Action**: Extract shared logic into reusable functions and delete redundant code.

## Execution Rules
- **Analyze**: Use `get_source_code` for context before any changes.
- **Precision**: Fix only if 100% sure.
- **Log**: Save results in `sonar_fix_report.md`.
- **Format**: Include Issue ID, Status (Fixed/Skipped), and Reason.