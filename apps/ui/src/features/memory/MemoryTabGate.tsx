import { MemoryTabLegacy } from "./MemoryTabLegacy";
import { MemoryTab as MemoryTabCommandDeck } from "./command-deck/MemoryTab";
import { useFeatureFlag } from "./useFeatureFlag";

interface MemoryTabGateProps {
  agentId: string;
}

export function MemoryTabGate({ agentId }: MemoryTabGateProps) {
  const on = useFeatureFlag("memory_tab_command_deck");
  return on ? (
    <MemoryTabCommandDeck agentId={agentId} />
  ) : (
    <MemoryTabLegacy agentId={agentId} />
  );
}
