// Page-level Memory route. Renders the Command Deck memory tab scoped to root.
import { MemoryTab } from "./command-deck/MemoryTab";

export function MemoryPage() {
  return <MemoryTab agentId="agent:root" />;
}
