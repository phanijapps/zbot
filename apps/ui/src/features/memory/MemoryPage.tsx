// Page-level Memory route component.
// Default: render the Command Deck MemoryTab. Users who explicitly toggled
// the flag OFF in Settings still get WebMemoryPanel.
import { MemoryTab as MemoryTabCommandDeck } from "./command-deck/MemoryTab";
import { WebMemoryPanel } from "./WebMemoryPanel";
import { useFeatureFlag } from "./useFeatureFlag";

export function MemoryPage() {
  const on = useFeatureFlag("memory_tab_command_deck", true);
  return on ? <MemoryTabCommandDeck agentId="agent:root" /> : <WebMemoryPanel />;
}
