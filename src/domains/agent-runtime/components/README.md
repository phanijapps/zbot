# Agent Runtime UI Components

Complete implementation of the conversation UI with thinking panel, based on our UX design.

## Components Created

### 1. Thinking Tab (`ThinkingTab.tsx`)
- Animated tab indicator showing agent thinking status
- Pulse animation when agent is working
- Shows tool count badge when completed
- Click to toggle thinking panel

### 2. Thinking Panel (`ThinkingPanel.tsx`)
- Right side panel (desktop) showing agent's thought process
- Sections: Execution Plan, Tool Calls, Reasoning
- Auto-opens on agent response, auto-collapses when done
- Responsive variants: Desktop (side), Tablet (bottom), Mobile (modal)

### 3. Plan Section (`PlanSection.tsx`)
- Checklist display when planning module is active
- Status indicators: pending (○), in-progress (⟳), completed (✓), failed (✗)
- Strikethrough for completed items

### 4. Tool Calls Section (`ToolCallsSection.tsx`)
- Minimal format: tool name only (e.g., `read`, `bash`)
- Status indicators with duration for completed tools
- Expandable detail view on click

### 5. Conversation List (`ConversationList.tsx`)
- Agent cards with identity (name, icon, model)
- Last message preview, timestamp, message count
- Empty state with "Start conversation" CTA
- Option for grouped-by-agent view

### 6. Conversation View (`ConversationView.tsx`)
- Chat interface with integrated ThinkingPanel
- Streaming message support
- Thinking tab in header with animated indicator
- Message input with keyboard shortcuts (Enter to send)
- Responsive variants for different screen sizes

### 7. Hook (`useStreamEvents.ts`)
- Handles stream events from AgentExecutor
- Auto-opens/collapses panel
- Accumulates plan items, tool calls, reasoning
- Manual panel control methods (toggle, open, close)

### 8. Types (`types.ts`)
- TypeScript definitions for all components
- Plan items, tool calls, thinking state
- Message with thinking metadata
- Conversation with agent data

## Animations Added to `index.css`

```css
/* Thinking panel animations */
@keyframes thinking-pulse      /* Gentle opacity pulse for emoji */
@keyframes thinking-breathe    /* Subtle scale breathe */
@keyframes panel-slide-in      /* Slide from right (desktop) */
@keyframes panel-slide-up      /* Slide from bottom (mobile) */
@keyframes bounce              /* Typing indicator dots */

Utility classes: .animate-thinking-pulse, .thinking-breathe,
.animate-panel-slide-in, .animate-panel-slide-up, .animate-fade-in
```

## File Structure

```
src/domains/agent-runtime/components/
├── index.ts                    # Exports all components
├── types.ts                    # TypeScript definitions
├── useStreamEvents.ts          # Hook for handling stream events
├── ThinkingTab.tsx             # Animated tab indicator
├── ThinkingPanel.tsx           # Main thinking panel (with variants)
├── PlanSection.tsx             # Checklist for planning module
├── ToolCallsSection.tsx        # Tool call display
├── ConversationList.tsx        # List of conversations with agent cards
└── ConversationView.tsx        # Chat interface with thinking panel
```

## Usage Example

```tsx
import {
  ConversationView,
  ConversationList,
  useStreamEvents,
} from "@/domains/agent-runtime/components";

function MyApp() {
  const [conversations, setConversations] = useState<ConversationWithAgent[]>([]);
  const [selectedConversation, setSelectedConversation] = useState<ConversationWithAgent | null>(null);
  const [messages, setMessages] = useState<MessageWithThinking[]>([]);

  const handleSendMessage = async (content: string) => {
    // Use conversationService.executeAgentStream()
    // Handle stream events with useStreamEvents hook
  };

  return (
    <div className="flex h-screen">
      {/* Conversation List - Left Sidebar */}
      <div className="w-80 border-r">
        <ConversationList
          conversations={conversations}
          selectedId={selectedConversation?.id}
          onSelect={setSelectedConversation}
          onNewChat={() => {/* Create new conversation */}}
        />
      </div>

      {/* Chat Area with Thinking Panel */}
      <div className="flex-1">
        <ConversationView
          conversation={selectedConversation}
          messages={messages}
          onSendMessage={handleSendMessage}
          onBack={() => setSelectedConversation(null)}
          onNewChat={() => {/* New chat */}}
        />
      </div>
    </div>
  );
}
```

## Integration with Agent Executor

To wire up streaming events:

```tsx
import { conversationService } from "@/domains/agent-runtime/services/ConversationService";
import { useStreamEvents } from "@/domains/agent-runtime/components";

function MyComponent() {
  const { state, handleEvent, reset, setCurrentMessage } = useStreamEvents();

  const sendMessage = async (content: string) => {
    reset();
    setCurrentMessage(messageId);

    await conversationService.executeAgentStream(
      conversationId,
      agentId,
      content,
      (event) => {
        handleEvent(event);  // This updates the thinking panel state
      }
    );
  };
}
```

## Responsive Behavior

| Screen Size | Panel Behavior |
|-------------|---------------|
| Desktop (>1024px) | Right side panel, 30% width |
| Tablet (768-1024px) | Bottom collapsible panel |
| Mobile (<768px) | Full-screen modal (tap to open) |

## State Flow

```
User sends message
    ↓
Agent starts working
    ↓
Tab animates (🧠 pulse)
    ↓
Panel auto-opens
    ↓
Events stream in:
  - Plan items appear (if planning module)
  - Tool calls show status (pending → running → done)
  - Reasoning blocks accumulate
    ↓
Agent finishes
    ↓
Animation stops
    ↓
Panel auto-collapses
    ↓
"[🧠 Used N tools]" badge remains
```

## Next Steps for Full Integration

1. Update `ConversationsPanel.tsx` to use the new components
2. Connect to Tauri commands (`execute_agent_stream`, etc.)
3. Integrate with `conversationService` for actual agent execution
4. Add error handling and retry logic
5. Implement actual provider/model configuration loading

## Known Limitations

- `ConversationView` is a reference implementation - needs integration with actual data
- `useStreamEvents` hook is ready but not yet connected to the AgentExecutor
- Responsive variants (Tablet/Mobile) need refinement
- Planning module integration is placeholder

## Build Status

✅ All components compile successfully
✅ TypeScript types verified
✅ CSS animations added to index.css
✅ Build passes without warnings (except chunk size)
