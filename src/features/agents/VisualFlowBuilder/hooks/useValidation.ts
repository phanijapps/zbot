// ============================================================================
// ZERO IDE - VALIDATION HOOK
// Hook for real-time validation of canvas state
// ============================================================================

import { useCallback, useMemo } from "react";
import type { CanvasState, BaseNode, ValidationResult, NodeData, SubagentNodeData } from "../types";

// -----------------------------------------------------------------------------
// Helper: Validate subagent node
// -----------------------------------------------------------------------------

function validateSubagentNode(node: BaseNode, data: NodeData): ValidationResult[] {
  const results: ValidationResult[] = [];

  if (node.type !== "subagent") {
    return results;
  }

  const subagentData = data as SubagentNodeData;

  // Check display name (subagentId is auto-generated from displayName)
  if (!subagentData.displayName || subagentData.displayName.trim() === "") {
    results.push({
      nodeId: node.id,
      type: "error",
      message: "Display name cannot be empty",
    });
  }

  // Check if subagent has been configured (has config object)
  const nodeDataRecord = data as unknown as Record<string, unknown>;
  const hasConfig = !!nodeDataRecord.config;
  if (!hasConfig) {
    results.push({
      nodeId: node.id,
      type: "warning",
      message: "Subagent not configured - fill in the properties panel",
    });
  }

  return results;
}

// This function is called in the validateState switch statement, so it's used

// -----------------------------------------------------------------------------
// Helper: Validate conditional node
// -----------------------------------------------------------------------------

function validateConditionalNode(node: BaseNode, data: NodeData): ValidationResult[] {
  const results: ValidationResult[] = [];

  if (node.type !== "conditional") {
    return results;
  }

  const conditionalData = data as unknown as Record<string, unknown>;

  // Check display name
  if (!conditionalData.displayName || (conditionalData.displayName as string).trim() === "") {
    results.push({
      nodeId: node.id,
      type: "error",
      message: "Display name cannot be empty",
    });
  }

  // Check conditions count
  if (!conditionalData.conditions || (conditionalData.conditions as unknown[]).length < 1) {
    results.push({
      nodeId: node.id,
      type: "error",
      message: "Conditional node must have at least 1 condition",
    });
  }

  return results;
}

// -----------------------------------------------------------------------------
// Helper: Validate start node
// -----------------------------------------------------------------------------

function validateStartNode(node: BaseNode): ValidationResult[] {
  const results: ValidationResult[] = [];

  if (node.type !== "start") {
    return results;
  }

  // Start nodes don't need much validation
  return results;
}

// -----------------------------------------------------------------------------
// Helper: Validate end node
// -----------------------------------------------------------------------------

function validateEndNode(node: BaseNode): ValidationResult[] {
  const results: ValidationResult[] = [];

  if (node.type !== "end") {
    return results;
  }

  // End nodes don't need validation
  return results;
}

// -----------------------------------------------------------------------------
// Hook
// -----------------------------------------------------------------------------

export function useValidation(state: CanvasState) {
  // -----------------------------------------------------------------------------
  // Validate all nodes
  // -----------------------------------------------------------------------------

  const validateState = useCallback((): ValidationResult[] => {
    const results: ValidationResult[] = [];

    // Check for duplicate display names
    const displayNames = new Map<string, string[]>();
    state.nodes.forEach((node) => {
      const name = node.data.displayName || "";
      if (name) {
        if (!displayNames.has(name)) {
          displayNames.set(name, []);
        }
        displayNames.get(name)!.push(node.id);
      }
    });

    // Add duplicate name errors
    displayNames.forEach((nodeIds, _name) => {
      if (nodeIds.length > 1) {
        nodeIds.forEach((nodeId) => {
          results.push({
            nodeId,
            type: "error",
            message: "Display name must be unique",
          });
        });
      }
    });

    // Validate each node based on its type
    state.nodes.forEach((node) => {
      switch (node.type) {
        case "start":
          results.push(...validateStartNode(node));
          break;
        case "end":
          results.push(...validateEndNode(node));
          break;
        case "conditional":
          results.push(...validateConditionalNode(node, node.data));
          break;
        case "subagent":
          results.push(...validateSubagentNode(node, node.data));
          break;
      }
    });

    // Check that flow has start and end nodes
    const hasStartNode = state.nodes.some((n) => n.type === "start");
    const hasEndNode = state.nodes.some((n) => n.type === "end");

    if (!hasStartNode) {
      results.push({
        type: "error",
        message: "Flow must have a Start event",
      });
    }

    if (!hasEndNode) {
      results.push({
        type: "warning",
        message: "Flow should have an End event",
      });
    }

    return results;
  }, [state.nodes]);

  // -----------------------------------------------------------------------------
  // Memoized validation results
  // -----------------------------------------------------------------------------

  const validationResults = useMemo(() => validateState(), [validateState]);

  // -----------------------------------------------------------------------------
  // Get validation for specific node
  // -----------------------------------------------------------------------------

  const getNodeValidation = useCallback(
    (nodeId: string): ValidationResult[] => {
      return validationResults.filter((v) => v.nodeId === nodeId);
    },
    [validationResults]
  );

  // -----------------------------------------------------------------------------
  // Check if node is valid
  // -----------------------------------------------------------------------------

  const isNodeValid = useCallback(
    (nodeId: string): boolean => {
      const nodeValidation = getNodeValidation(nodeId);
      return !nodeValidation.some((v) => v.type === "error");
    },
    [getNodeValidation]
  );

  // -----------------------------------------------------------------------------
  // Get overall validation status
  // -----------------------------------------------------------------------------

  const overallStatus = useMemo(() => {
    const errors = validationResults.filter((v) => v.type === "error");
    const warnings = validationResults.filter((v) => v.type === "warning");

    if (errors.length > 0) {
      return "error" as const;
    }
    if (warnings.length > 0) {
      return "warning" as const;
    }
    return "valid" as const;
  }, [validationResults]);

  // -----------------------------------------------------------------------------
  // Get error/warning counts
  // -----------------------------------------------------------------------------

  const counts = useMemo(() => {
    return {
      errors: validationResults.filter((v) => v.type === "error").length,
      warnings: validationResults.filter((v) => v.type === "warning").length,
      info: validationResults.filter((v) => v.type === "info").length,
    };
  }, [validationResults]);

  return {
    validationResults,
    getNodeValidation,
    isNodeValid,
    overallStatus,
    counts,
  };
}
