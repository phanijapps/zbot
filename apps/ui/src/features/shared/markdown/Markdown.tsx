// =============================================================================
// Shared markdown renderer with GFM + LaTeX math support.
//
// All agent responses and artifact previews funnel through this component
// so the plugin set stays consistent: GitHub-flavored markdown (tables,
// strikethrough, task lists, autolinks) plus inline `$…$` and block `$$…$$`
// LaTeX via remark-math + rehype-katex.
// =============================================================================

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import "katex/dist/katex.min.css";

export interface MarkdownProps {
  children: string;
  /** Extra class on the wrapping div. */
  className?: string;
}

export function Markdown({ children, className }: MarkdownProps) {
  return (
    <div className={className}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkMath]}
        rehypePlugins={[rehypeKatex]}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}
