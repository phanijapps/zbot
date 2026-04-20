// =============================================================================
// ResearchMessages — per-message bubbles + z-icon + copy-to-clipboard.
//
// Two presentations: user (right-aligned pill) and assistant (left-aligned
// with a z-brand avatar on top of the message). Both expose a copy button
// that copies the raw message text (markdown, for assistants) to the
// clipboard. Used by ResearchPage.MainColumn when hydrating history and
// reused by AgentTurnBlock for the live Respond body.
// =============================================================================

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { CopyButton } from "../shared/copyButton";

export { CopyButton } from "../shared/copyButton";

/** Small brand avatar — z-bot icon. Swapped via CSS for dark/light themes. */
export function AgentAvatar() {
  return (
    <img
      className="research-msg__avatar"
      src="/zbot_icon_dark.svg"
      alt="z-Bot"
      width={20}
      height={20}
      aria-hidden="true"
    />
  );
}

interface UserMessageProps {
  content: string;
}

export function UserMessage({ content }: UserMessageProps) {
  return (
    <div className="research-msg research-msg--user" data-copy-host="true">
      <div className="research-msg__bubble research-page__user-bubble">
        {content}
      </div>
      <CopyButton text={content} label="Copy question" />
    </div>
  );
}

interface AssistantMessageProps {
  content: string;
}

/** Hydrated-history assistant message. Live responses render through
 *  AgentTurnBlock's Respond body (which uses the same avatar + copy slot). */
export function AssistantMessage({ content }: AssistantMessageProps) {
  return (
    <div className="research-msg research-msg--assistant" data-copy-host="true">
      <AgentAvatar />
      <div className="research-msg__body research-page__assistant">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
      </div>
      <CopyButton text={content} label="Copy answer" />
    </div>
  );
}
