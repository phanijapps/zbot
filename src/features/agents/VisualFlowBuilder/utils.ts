// ============================================================================
// VISUAL FLOW BUILDER - UTILITIES
// Helper functions for the visual flow builder
// ============================================================================

// -----------------------------------------------------------------------------
// Calculate Bezier Curve Path
// -----------------------------------------------------------------------------

/**
 * Calculate a smooth bezier curve path between two points
 * @param startX - Starting X coordinate
 * @param startY - Starting Y coordinate
 * @param endX - Ending X coordinate
 * @param endY - Ending Y coordinate
 * @param curvature - Control point curvature (default: 0.5)
 * @returns SVG path string
 */
export function calculateBezierPath(
  startX: number,
  startY: number,
  endX: number,
  endY: number,
  curvature: number = 0.5
): string {
  const deltaX = endX - startX;

  // Control points for smooth S-curve
  const controlPoint1X = startX + Math.max(deltaX * curvature, 50);
  const controlPoint1Y = startY;

  const controlPoint2X = endX - Math.max(deltaX * curvature, 50);
  const controlPoint2Y = endY;

  return `M ${startX} ${startY} C ${controlPoint1X} ${controlPoint1Y}, ${controlPoint2X} ${controlPoint2Y}, ${endX} ${endY}`;
}

// -----------------------------------------------------------------------------
// Generate Unique ID
// -----------------------------------------------------------------------------

/**
 * Generate a unique ID for nodes, connections, etc.
 * @param prefix - Optional prefix for the ID
 * @returns Unique ID string
 */
export function generateId(prefix: string = "id"): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).substring(2, 11)}`;
}

// -----------------------------------------------------------------------------
// Snap to Grid
// -----------------------------------------------------------------------------

/**
 * Snap a coordinate value to the nearest grid point
 * @param value - Coordinate value to snap
 * @param gridSize - Size of the grid (default: 20)
 * @returns Snapped coordinate value
 */
export function snapToGrid(value: number, gridSize: number = 20): number {
  return Math.round(value / gridSize) * gridSize;
}

// -----------------------------------------------------------------------------
// Calculate Distance
// -----------------------------------------------------------------------------

/**
 * Calculate the Euclidean distance between two points
 * @param x1 - First point X
 * @param y1 - First point Y
 * @param x2 - Second point X
 * @param y2 - Second point Y
 * @returns Distance between the points
 */
export function distance(x1: number, y1: number, x2: number, y2: number): number {
  return Math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2);
}

// -----------------------------------------------------------------------------
// Clamp Value
// -----------------------------------------------------------------------------

/**
 * Clamp a value between a minimum and maximum
 * @param value - Value to clamp
 * @param min - Minimum value
 * @param max - Maximum value
 * @returns Clamped value
 */
export function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

// -----------------------------------------------------------------------------
// Check if Point is in Rectangle
// -----------------------------------------------------------------------------

/**
 * Check if a point is inside a rectangle
 * @param pointX - Point X coordinate
 * @param pointY - Point Y coordinate
 * @param rectX - Rectangle X coordinate (top-left)
 * @param rectY - Rectangle Y coordinate (top-left)
 * @param rectWidth - Rectangle width
 * @param rectHeight - Rectangle height
 * @returns True if point is inside rectangle
 */
export function isPointInRect(
  pointX: number,
  pointY: number,
  rectX: number,
  rectY: number,
  rectWidth: number,
  rectHeight: number
): boolean {
  return (
    pointX >= rectX &&
    pointX <= rectX + rectWidth &&
    pointY >= rectY &&
    pointY <= rectY + rectHeight
  );
}

// -----------------------------------------------------------------------------
// Check if Point is Near Line
// -----------------------------------------------------------------------------

/**
 * Check if a point is near a line segment (for connection selection)
 * @param pointX - Point X coordinate
 * @param pointY - Point Y coordinate
 * @param lineStartX - Line start X
 * @param lineStartY - Line start Y
 * @param lineEndX - Line end X
 * @param lineEndY - Line end Y
 * @param threshold - Distance threshold (default: 5)
 * @returns True if point is near the line
 */
export function isPointNearLine(
  pointX: number,
  pointY: number,
  lineStartX: number,
  lineStartY: number,
  lineEndX: number,
  lineEndY: number,
  threshold: number = 5
): boolean {
  const A = pointX - lineStartX;
  const B = pointY - lineStartY;
  const C = lineEndX - lineStartX;
  const D = lineEndY - lineStartY;

  const dot = A * C + B * D;
  const lenSq = C * C + D * D;

  let param = -1;
  if (lenSq !== 0) {
    param = dot / lenSq;
  }

  let xx: number;
  let yy: number;

  if (param < 0) {
    xx = lineStartX;
    yy = lineStartY;
  } else if (param > 1) {
    xx = lineEndX;
    yy = lineEndY;
  } else {
    xx = lineStartX + param * C;
    yy = lineStartY + param * D;
  }

  const dx = pointX - xx;
  const dy = pointY - yy;

  return Math.sqrt(dx * dx + dy * dy) < threshold;
}

// -----------------------------------------------------------------------------
// Format Number with Suffix
// -----------------------------------------------------------------------------

/**
 * Format a number with a suffix (1st, 2nd, 3rd, 4th, etc.)
 * @param num - Number to format
 * @returns Formatted string with suffix
 */
export function formatNumberWithSuffix(num: number): string {
  const suffixes = ["th", "st", "nd", "rd"];
  const value = num % 100;
  const suffix = suffixes[(value - 20) % 10] || suffixes[value] || suffixes[0];
  return `${num}${suffix}`;
}

// -----------------------------------------------------------------------------
// Truncate Text
// -----------------------------------------------------------------------------

/**
 * Truncate text to a maximum length and add ellipsis
 * @param text - Text to truncate
 * @param maxLength - Maximum length
 * @returns Truncated text
 */
export function truncateText(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength - 3) + "...";
}

// -----------------------------------------------------------------------------
// Deep Clone
// -----------------------------------------------------------------------------

/**
 * Create a deep clone of an object (JSON-safe)
 * @param obj - Object to clone
 * @returns Cloned object
 */
export function deepClone<T>(obj: T): T {
  return JSON.parse(JSON.stringify(obj));
}

// -----------------------------------------------------------------------------
// Debounce Function
// -----------------------------------------------------------------------------

/**
 * Create a debounced function that delays invoking func until after wait milliseconds
 * @param func - Function to debounce
 * @param wait - Milliseconds to wait
 * @returns Debounced function
 */
export function debounce<T extends (...args: unknown[]) => unknown>(
  func: T,
  wait: number
): (...args: Parameters<T>) => void {
  let timeout: ReturnType<typeof setTimeout> | null = null;

  return function executedFunction(...args: Parameters<T>) {
    const later = () => {
      timeout = null;
      func(...args);
    };

    if (timeout) {
      clearTimeout(timeout);
    }
    timeout = setTimeout(later, wait);
  };
}

// -----------------------------------------------------------------------------
// Throttle Function
// -----------------------------------------------------------------------------

/**
 * Create a throttled function that only invokes func at most once per every wait milliseconds
 * @param func - Function to throttle
 * @param wait - Milliseconds to wait
 * @returns Throttled function
 */
export function throttle<T extends (...args: unknown[]) => unknown>(
  func: T,
  wait: number
): (...args: Parameters<T>) => void {
  let inThrottle = false;

  return function executedFunction(...args: Parameters<T>) {
    if (!inThrottle) {
      func(...args);
      inThrottle = true;
      setTimeout(() => {
        inThrottle = false;
      }, wait);
    }
  };
}
