// ============================================================================
// VISUAL FLOW BUILDER - VALIDATION HOOK
// Hook for real-time validation of canvas state
// ============================================================================

import { useCallback, useMemo } from "react";
import type { CanvasState, BaseNode, ValidationResult, NodeData, AgentNodeData } from "../types";
import { VALIDATION_MESSAGES } from "../constants";

// -----------------------------------------------------------------------------
// Helper: Validate agent node
// -----------------------------------------------------------------------------

function validateAgentNode(node: BaseNode, data: NodeData): ValidationResult[] {
  const results: ValidationResult[] = [];

  if (node.type !== "agent") {
    return results;
  }

  const agentData = data as AgentNodeData;

  // Check display name
  if (!agentData.displayName || agentData.displayName.trim() === "") {
    results.push({
      nodeId: node.id,
      type: "error",
      message: VALIDATION_MESSAGES.EMPTY_DISPLAY_NAME,
    });
  }

  // Check provider
  if (!agentData.providerId) {
    results.push({
      nodeId: node.id,
      type: "error",
      message: VALIDATION_MESSAGES.NO_PROVIDER,
    });
  }

  // Check model
  if (!agentData.model) {
    results.push({
      nodeId: node.id,
      type: "error",
      message: VALIDATION_MESSAGES.NO_MODEL,
    });
  }

  // Warning: no tools
  if (!agentData.tools || agentData.tools.length === 0) {
    results.push({
      nodeId: node.id,
      type: "warning",
      message: VALIDATION_MESSAGES.NO_TOOLS,
    });
  }

  return results;
}

// -----------------------------------------------------------------------------
// Helper: Validate parallel node
// -----------------------------------------------------------------------------

function validateParallelNode(node: BaseNode, data: NodeData): ValidationResult[] {
  const results: ValidationResult[] = [];

  if (node.type !== "parallel") {
    return results;
  }

  const parallelData = data as unknown as Record<string, unknown>;

  // Check display name
  if (!parallelData.displayName || (parallelData.displayName as string).trim() === "") {
    results.push({
      nodeId: node.id,
      type: "error",
      message: VALIDATION_MESSAGES.EMPTY_DISPLAY_NAME,
    });
  }

  // Check subagents count
  if (!parallelData.subagents || (parallelData.subagents as string[]).length < 2) {
    results.push({
      nodeId: node.id,
      type: "error",
      message: VALIDATION_MESSAGES.NO_SUBAGENTS,
    });
  }

  return results;
}

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
      message: VALIDATION_MESSAGES.EMPTY_DISPLAY_NAME,
    });
  }

  // Check conditions count
  if (!conditionalData.conditions || (conditionalData.conditions as unknown[]).length < 2) {
    results.push({
      nodeId: node.id,
      type: "error",
      message: VALIDATION_MESSAGES.NO_CONDITIONS,
    });
  }

  return results;
}

// -----------------------------------------------------------------------------
// Helper: Validate loop node
// -----------------------------------------------------------------------------

function validateLoopNode(node: BaseNode, data: NodeData): ValidationResult[] {
  const results: ValidationResult[] = [];

  if (node.type !== "loop") {
    return results;
  }

  const loopData = data as unknown as Record<string, unknown>;

  // Check display name
  if (!loopData.displayName || (loopData.displayName as string).trim() === "") {
    results.push({
      nodeId: node.id,
      type: "error",
      message: VALIDATION_MESSAGES.EMPTY_DISPLAY_NAME,
    });
  }

  // Check exit condition
  if (!loopData.exitCondition || (loopData.exitCondition as string).trim() === "") {
    results.push({
      nodeId: node.id,
      type: "error",
      message: VALIDATION_MESSAGES.NO_EXIT_CONDITION,
    });
  }

  // Validate max iterations
  if (loopData.maxIterations !== undefined && (loopData.maxIterations as number) < 1) {
    results.push({
      nodeId: node.id,
      type: "error",
      message: "Max iterations must be at least 1",
    });
  }

  return results;
}

// -----------------------------------------------------------------------------
// Helper: Validate sequential node
// -----------------------------------------------------------------------------

function validateSequentialNode(node: BaseNode, data: NodeData): ValidationResult[] {
  const results: ValidationResult[] = [];

  if (node.type !== "sequential") {
    return results;
  }

  const sequentialData = data as unknown as Record<string, unknown>;

  // Check display name
  if (!sequentialData.displayName || (sequentialData.displayName as string).trim() === "") {
    results.push({
      nodeId: node.id,
      type: "error",
      message: VALIDATION_MESSAGES.EMPTY_DISPLAY_NAME,
    });
  }

  // Check subtasks count
  if (!sequentialData.subtasks || (sequentialData.subtasks as string[]).length === 0) {
    results.push({
      nodeId: node.id,
      type: "warning",
      message: "Sequential node has no subtasks defined",
    });
  }

  return results;
}

// -----------------------------------------------------------------------------
// Helper: Validate aggregator node
// -----------------------------------------------------------------------------

function validateAggregatorNode(node: BaseNode, data: NodeData): ValidationResult[] {
  const results: ValidationResult[] = [];

  if (node.type !== "aggregator") {
    return results;
  }

  const aggregatorData = data as unknown as Record<string, unknown>;

  // Check display name
  if (!aggregatorData.displayName || (aggregatorData.displayName as string).trim() === "") {
    results.push({
      nodeId: node.id,
      type: "error",
      message: VALIDATION_MESSAGES.EMPTY_DISPLAY_NAME,
    });
  }

  // Check strategy
  if (!aggregatorData.strategy) {
    results.push({
      nodeId: node.id,
      type: "warning",
      message: "No merge strategy selected",
    });
  }

  return results;
}

// -----------------------------------------------------------------------------
// Helper: Validate subtask node
// -----------------------------------------------------------------------------

function validateSubtaskNode(node: BaseNode, data: NodeData): ValidationResult[] {
  const results: ValidationResult[] = [];

  if (node.type !== "subtask") {
    return results;
  }

  const subtaskData = data as unknown as Record<string, unknown>;

  // Check display name
  if (!subtaskData.displayName || (subtaskData.displayName as string).trim() === "") {
    results.push({
      nodeId: node.id,
      type: "error",
      message: VALIDATION_MESSAGES.EMPTY_DISPLAY_NAME,
    });
  }

  // Check tasks
  if (!subtaskData.tasks || (subtaskData.tasks as string[]).length === 0) {
    results.push({
      nodeId: node.id,
      type: "warning",
      message: "Subtask has no tasks defined",
    });
  }

  // Check goal
  if (!subtaskData.goal || (subtaskData.goal as string).trim() === "") {
    results.push({
      nodeId: node.id,
      type: "error",
      message: "Subtask must have a goal defined",
    });
  }

  // Check agent reference
  if (!subtaskData.agentNodeId) {
    results.push({
      nodeId: node.id,
      type: "error",
      message: "Subtask must reference an agent configuration",
    });
  }

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
            message: VALIDATION_MESSAGES.DUPLICATE_NAME,
          });
        });
      }
    });

    // Validate each node based on its type
    state.nodes.forEach((node) => {
      switch (node.type) {
        case "agent":
          results.push(...validateAgentNode(node, node.data));
          break;
        case "parallel":
          results.push(...validateParallelNode(node, node.data));
          break;
        case "conditional":
          results.push(...validateConditionalNode(node, node.data));
          break;
        case "loop":
          results.push(...validateLoopNode(node, node.data));
          break;
        case "sequential":
          results.push(...validateSequentialNode(node, node.data));
          break;
        case "aggregator":
          results.push(...validateAggregatorNode(node, node.data));
          break;
        case "subtask":
          results.push(...validateSubtaskNode(node, node.data));
          break;
        case "trigger":
          // Trigger nodes don't need validation
          break;
      }
    });

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
