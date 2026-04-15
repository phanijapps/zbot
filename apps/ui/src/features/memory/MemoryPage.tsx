// Page-level Memory route component.
// When the memory_tab_command_deck flag is on, renders the new Command Deck
// MemoryTab (self-selects the first ward, agent scoped to "agent:root").
// When off, renders the existing WebMemoryPanel unchanged.
import { MemoryTab as MemoryTabCommandDeck } from "./command-deck/MemoryTab";
import { WebMemoryPanel } from "./WebMemoryPanel";
import { useFeatureFlag } from "./useFeatureFlag";

export function MemoryPage() {
  const on = useFeatureFlag("memory_tab_command_deck");
  return on ? <MemoryTabCommandDeck agentId="agent:root" /> : <WebMemoryPanel />;
}
