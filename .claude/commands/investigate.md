---
description: Investigative Root Cause Analysis (RCA) and Permanent Fix using SonarQube MCP
---

# Root Cause & Permanent Fix

Follow these steps to find and fix issues once and for all.

## 1. Context Loading
- Read `CLAUDE.md`, `AGENTS.md`, and the `memory-bank/` directory.
- Use `sonarqube.search_issues` to see if the issue is already flagged.

## 2. Investigation
- **Analyze Intent**: Define exactly what the user wants to achieve.
- **Locate**: Identify files or modules where the problem lives.
- **Verify**: Confirm the **Root Cause** through code analysis or local tests.

## 3. Solution Design
- **Brainstorm**: Create a fix that prevents the issue from returning.
- **Compliance Check**: Ensure the plan follows SonarQube rules and project standards.
- **Approval**: Present the plan to the user. **Wait for "OK" to proceed.**

## 4. Execution & Validation
- **Fix**: Apply the code changes.
- **Test**: Run local tests to ensure high **Code Coverage**.
- **Sonar Scan**: Use `sonarqube` MCP to verify no new issues were created. This has to be run on the local repo. If you cannot that is ok.
- **Final Log**: Update `memory-bank/` if the system architecture changed.