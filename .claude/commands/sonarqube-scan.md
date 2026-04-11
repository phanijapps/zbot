---
description: Used to only to scan the current directory to produce are report.
---
Use the `sonarqube` MCP and local shell to run a full scan on the current directory. 

1. **Scan**: Trigger `sonar-scanner` for the current project branch.
2. **Analyze**: Retrieve all Security, Reliability, Maintainability, and Duplication issues found.
3. **Report**: Create a file named `memory-bank/sonar_scan_report.md`.
4. **Content**: For every issue, list:
   - Type (Bug, Vulnerability, etc.)
   - Severity
   - File Path and Line Number
   - Brief description of the issue

Do not apply any fixes yet. Just provide the report.