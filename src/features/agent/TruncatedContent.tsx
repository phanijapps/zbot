// ============================================================================
// TRUNCATED CONTENT
// Displays content with truncation and expand functionality
// Designed for easy migration to server-side lazy loading
// ============================================================================

import { useState, useMemo, useCallback } from "react";
import { ChevronDown, Loader2 } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

// ============================================================================
// Types
// ============================================================================

export interface TruncatedContentProps {
  /** Unique identifier for this content (message ID) */
  id: string;
  /** The content to display */
  content: string;
  /** Maximum words before truncation (default: 400) */
  maxWords?: number;
  /**
   * Optional async loader for full content (for server-side lazy loading)
   * If not provided, uses the content prop directly
   */
  loadFullContent?: (id: string) => Promise<string>;
  /** Custom class name for the container */
  className?: string;
}

interface TruncationResult {
  text: string;
  isTruncated: boolean;
  wordCount: number;
  totalWords: number;
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Truncate content at a clean boundary (sentence or paragraph end preferred)
 */
function truncateAtCleanBoundary(content: string, maxWords: number): TruncationResult {
  const words = content.split(/\s+/);
  const totalWords = words.length;

  if (totalWords <= maxWords) {
    return { text: content, isTruncated: false, wordCount: totalWords, totalWords };
  }

  // Get roughly maxWords worth of content
  const roughTruncated = words.slice(0, maxWords).join(' ');

  // Try to find a clean break point (end of sentence or paragraph)
  // Look for the last sentence-ending punctuation followed by space or end
  const cleanBreakMatch = roughTruncated.match(/^([\s\S]*[.!?])\s+\S*$/);

  let truncatedText: string;
  if (cleanBreakMatch && cleanBreakMatch[1].length > roughTruncated.length * 0.7) {
    // Found a clean break that's at least 70% of the target length
    truncatedText = cleanBreakMatch[1];
  } else {
    // No clean break, just use word boundary
    truncatedText = roughTruncated;
  }

  // Ensure we don't break markdown syntax
  truncatedText = closeOpenMarkdownSyntax(truncatedText);

  return {
    text: truncatedText,
    isTruncated: true,
    wordCount: truncatedText.split(/\s+/).length,
    totalWords,
  };
}

/**
 * Close any open markdown syntax to prevent rendering issues
 */
function closeOpenMarkdownSyntax(text: string): string {
  let result = text;

  // Count open code blocks (```)
  const codeBlockMatches = result.match(/```/g);
  if (codeBlockMatches && codeBlockMatches.length % 2 !== 0) {
    result += '\n```';
  }

  // Count open inline code (`)
  const inlineCodeMatches = result.match(/(?<!`)`(?!`)/g);
  if (inlineCodeMatches && inlineCodeMatches.length % 2 !== 0) {
    result += '`';
  }

  // Count open bold (**)
  const boldMatches = result.match(/\*\*/g);
  if (boldMatches && boldMatches.length % 2 !== 0) {
    result += '**';
  }

  // Count open italic (single *)
  const italicMatches = result.match(/(?<!\*)\*(?!\*)/g);
  if (italicMatches && italicMatches.length % 2 !== 0) {
    result += '*';
  }

  return result;
}

// ============================================================================
// Component
// ============================================================================

export function TruncatedContent({
  id,
  content,
  maxWords = 400,
  loadFullContent,
  className = "",
}: TruncatedContentProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [fullContent, setFullContent] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  // Memoize truncation result
  const truncation = useMemo(
    () => truncateAtCleanBoundary(content, maxWords),
    [content, maxWords]
  );

  // Handle expand/collapse
  const handleToggle = useCallback(async () => {
    if (isExpanded) {
      // Collapsing - just toggle state
      setIsExpanded(false);
      return;
    }

    // Expanding
    if (loadFullContent && !fullContent) {
      // Server-side loading path
      setIsLoading(true);
      setLoadError(null);
      try {
        const loaded = await loadFullContent(id);
        setFullContent(loaded);
        setIsExpanded(true);
      } catch (error) {
        setLoadError(error instanceof Error ? error.message : "Failed to load content");
      } finally {
        setIsLoading(false);
      }
    } else {
      // Client-side - content already available
      setIsExpanded(true);
    }
  }, [isExpanded, loadFullContent, fullContent, id]);

  // Determine what content to display
  const displayContent = isExpanded
    ? (fullContent ?? content)
    : truncation.text;

  // Don't show expand button if not truncated
  if (!truncation.isTruncated) {
    return (
      <div className={`prose prose-sm dark:prose-invert max-w-none ${className}`}>
        <ReactMarkdown remarkPlugins={[remarkGfm]}>
          {content}
        </ReactMarkdown>
      </div>
    );
  }

  return (
    <div className={className}>
      {/* Content */}
      <div className="prose prose-sm dark:prose-invert max-w-none text-sm prose-headings:mt-3 prose-headings:mb-2 prose-p:my-1 prose-pre:bg-[var(--muted)] prose-pre:border prose-pre:border-[var(--border)] prose-code:text-[var(--primary)] prose-code:bg-[var(--muted)] prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:before:content-none prose-code:after:content-none">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>
          {displayContent}
        </ReactMarkdown>
      </div>

      {/* Expand button - only shown when collapsed */}
      {!isExpanded && (
        <div className="mt-2 pt-2 border-t border-current/10">
          {loadError ? (
            <div className="text-xs text-red-500 mb-1">{loadError}</div>
          ) : null}

          <button
            onClick={handleToggle}
            disabled={isLoading}
            className="flex items-center gap-1.5 text-xs font-medium text-[var(--primary)] hover:text-[var(--primary)]/80 transition-colors disabled:opacity-50"
          >
            {isLoading ? (
              <>
                <Loader2 className="w-3 h-3 animate-spin" />
                Loading...
              </>
            ) : (
              <>
                <ChevronDown className="w-3 h-3" />
                Show more ({truncation.totalWords - truncation.wordCount} more words)
              </>
            )}
          </button>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Export helper for server-side integration
// ============================================================================

/**
 * Factory to create a loadFullContent function for server-side loading
 * Usage: const loader = createContentLoader(transport);
 *        <TruncatedContent loadFullContent={loader} ... />
 */
export function createContentLoader(
  fetchFn: (messageId: string) => Promise<{ content: string }>
): (id: string) => Promise<string> {
  return async (id: string) => {
    const result = await fetchFn(id);
    return result.content;
  };
}
