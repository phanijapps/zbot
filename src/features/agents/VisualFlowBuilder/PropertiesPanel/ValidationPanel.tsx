// ============================================================================
// VISUAL FLOW BUILDER - VALIDATION PANEL
// Real-time validation display for nodes and canvas
// ============================================================================

import { memo } from "react";
import type { ValidationResult } from "../types";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface ValidationPanelProps {
  validationResults: ValidationResult[];
  nodeId?: string | null;
}

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const CheckIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M20 6 9 17l-5-5" />
  </svg>
);

const ErrorIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="10" />
    <path d="M12 8v4M12 16h.01" />
  </svg>
);

const WarningIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" />
    <path d="M12 9v4" />
    <path d="M12 17h.01" />
  </svg>
);

const InfoIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="10" />
    <path d="M12 16v-4" />
    <path d="M12 8h.01" />
  </svg>
);

// -----------------------------------------------------------------------------
// Validation Item Component
// -----------------------------------------------------------------------------

interface ValidationItemProps {
  result: ValidationResult;
}

const ValidationItem = memo(({ result }: ValidationItemProps) => {
  const getStyles = () => {
    switch (result.type) {
      case "error":
        return {
          bg: "bg-red-500/10",
          border: "border-red-500/20",
          text: "text-red-300",
          iconBg: "bg-red-500/20",
          icon: <ErrorIcon />,
        };
      case "warning":
        return {
          bg: "bg-yellow-500/10",
          border: "border-yellow-500/20",
          text: "text-yellow-300",
          iconBg: "bg-yellow-500/20",
          icon: <WarningIcon />,
        };
      case "info":
        return {
          bg: "bg-blue-500/10",
          border: "border-blue-500/20",
          text: "text-blue-300",
          iconBg: "bg-blue-500/20",
          icon: <InfoIcon />,
        };
      default:
        return {
          bg: "bg-gray-500/10",
          border: "border-gray-500/20",
          text: "text-gray-300",
          iconBg: "bg-gray-500/20",
          icon: <InfoIcon />,
        };
    }
  };

  const styles = getStyles();

  return (
    <div className={`flex items-start gap-2 p-2 rounded ${styles.bg} border ${styles.border}`}>
      <span className={`p-1 rounded ${styles.iconBg} ${styles.text}`}>
        {styles.icon}
      </span>
      <p className={`text-[10px] ${styles.text} flex-1`}>
        {result.message}
      </p>
    </div>
  );
});

ValidationItem.displayName = "ValidationItem";

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export const ValidationPanel = memo(({ validationResults, nodeId }: ValidationPanelProps) => {
  // Filter results by nodeId if provided
  const filteredResults = nodeId
    ? validationResults.filter((r) => !r.nodeId || r.nodeId === nodeId)
    : validationResults;

  // Count by type
  const errorCount = filteredResults.filter((r) => r.type === "error").length;
  const warningCount = filteredResults.filter((r) => r.type === "warning").length;

  const hasErrors = errorCount > 0;
  const hasWarnings = warningCount > 0;
  const hasIssues = hasErrors || hasWarnings;
  const hasResults = filteredResults.length > 0;

  return (
    <div className="space-y-3">
      {/* Status Summary */}
      <div className={`p-3 rounded-lg border ${
        hasErrors
          ? "bg-red-500/10 border-red-500/20"
          : hasWarnings
            ? "bg-yellow-500/10 border-yellow-500/20"
            : "bg-green-500/10 border-green-500/20"
      }`}>
        <div className="flex items-center gap-2">
          <span className={`p-1 rounded ${
            hasErrors
              ? "bg-red-500/20 text-red-300"
              : hasWarnings
                ? "bg-yellow-500/20 text-yellow-300"
                : "bg-green-500/20 text-green-300"
          }`}>
            <CheckIcon />
          </span>
          <div className="flex-1">
            <p className={`text-xs font-medium ${
              hasErrors
                ? "text-red-300"
                : hasWarnings
                  ? "text-yellow-300"
                  : "text-green-300"
            }`}>
              {hasErrors
                ? `${errorCount} error${errorCount > 1 ? "s" : ""} found`
                : hasWarnings
                  ? `${warningCount} warning${warningCount > 1 ? "s" : ""}`
                  : "All configurations valid"}
            </p>
            {hasIssues && (
              <p className="text-[10px] text-gray-500">
                {errorCount > 0 && `${errorCount} error${errorCount > 1 ? "s" : ""} • `}
                {warningCount > 0 && `${warningCount} warning${warningCount > 1 ? "s" : ""}`}
              </p>
            )}
          </div>
        </div>
      </div>

      {/* Validation Items */}
      {hasResults && (
        <div className="space-y-2">
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide">
            Validation Details
          </h3>
          <div className="space-y-2 max-h-[200px] overflow-y-auto">
            {filteredResults.map((result, index) => (
              <ValidationItem key={`${result.nodeId}-${index}`} result={result} />
            ))}
          </div>
        </div>
      )}

      {/* No Issues Message */}
      {!hasResults && !hasIssues && (
        <div className="p-3 rounded-lg bg-green-500/10 border border-green-500/20">
          <div className="flex items-center gap-2">
            <span className="p-1 rounded bg-green-500/20 text-green-300">
              <CheckIcon />
            </span>
            <p className="text-xs text-green-300">No validation issues found</p>
          </div>
        </div>
      )}
    </div>
  );
});

ValidationPanel.displayName = "ValidationPanel";
