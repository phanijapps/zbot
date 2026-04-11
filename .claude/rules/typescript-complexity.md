# TypeScript Complexity Rules

## Cognitive Complexity
- Keep function cognitive complexity under 15 (SonarQube threshold)
- This applies to all functions: components, hooks, event handlers, utilities

## Event Handlers
- Switch statements with more than 5 cases MUST extract each case into a named function
- Pattern:
  ```typescript
  // BAD — complexity explodes
  function handleEvent(event) {
    switch (event.type) {
      case "token": { /* 20 lines */ break; }
      case "tool_call": { /* 30 lines */ break; }
      // ...
    }
  }

  // GOOD — flat dispatcher
  function handleEvent(event) {
    switch (event.type) {
      case "token": return handleTokenEvent(event, ctx);
      case "tool_call": return handleToolCallEvent(event, ctx);
    }
  }
  ```

## React Hooks
- If a custom hook exceeds 100 lines, extract sub-hooks or helper functions
- Use a context object (interface) to pass shared state to extracted handlers:
  ```typescript
  interface EventHandlerCtx {
    setMessages: React.Dispatch<...>;
    bufferRef: React.MutableRefObject<string>;
    // ...
  }
  ```

## Nesting
- Maximum 4 levels of nested functions (SonarQube S2004)
- Extract inner functions to module scope when they don't need closure variables

## Components
- If a component exceeds 200 lines of JSX, extract sub-components
- Each tab/panel/section in a settings page should be its own component
