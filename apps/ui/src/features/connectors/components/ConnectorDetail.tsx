// ============================================================================
// CONNECTOR DETAIL
// Tab-based detail view for a selected connector
// ============================================================================

import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/shared/ui/tabs";
import type { ConnectorResponse } from "@/services/transport/types";
import { OverviewTab } from "./tabs/OverviewTab";
import { InboundTab } from "./tabs/InboundTab";
import { OutboundTab } from "./tabs/OutboundTab";
import { MetadataTab } from "./tabs/MetadataTab";

interface ConnectorDetailProps {
  connector: ConnectorResponse;
  apiBase: string;
  onUpdate: () => void;
}

export function ConnectorDetail({ connector, apiBase, onUpdate }: ConnectorDetailProps) {
  return (
    <Tabs defaultValue="overview" className="h-full flex flex-col">
      <TabsList className="bg-[var(--muted)] self-start">
        <TabsTrigger
          value="overview"
          className="data-[state=active]:bg-[var(--card)] data-[state=active]:text-[var(--foreground)] data-[state=active]:shadow-sm text-[var(--muted-foreground)]"
        >
          Overview
        </TabsTrigger>
        <TabsTrigger
          value="inbound"
          className="data-[state=active]:bg-[var(--card)] data-[state=active]:text-[var(--foreground)] data-[state=active]:shadow-sm text-[var(--muted-foreground)]"
        >
          Inbound
        </TabsTrigger>
        <TabsTrigger
          value="outbound"
          className="data-[state=active]:bg-[var(--card)] data-[state=active]:text-[var(--foreground)] data-[state=active]:shadow-sm text-[var(--muted-foreground)]"
        >
          Outbound
        </TabsTrigger>
        <TabsTrigger
          value="metadata"
          className="data-[state=active]:bg-[var(--card)] data-[state=active]:text-[var(--foreground)] data-[state=active]:shadow-sm text-[var(--muted-foreground)]"
        >
          Metadata
        </TabsTrigger>
      </TabsList>

      <TabsContent value="overview" className="pt-5 flex-1 overflow-auto">
        <OverviewTab connector={connector} />
      </TabsContent>

      <TabsContent value="inbound" className="pt-5 flex-1 overflow-auto">
        <InboundTab connector={connector} apiBase={apiBase} onUpdate={onUpdate} />
      </TabsContent>

      <TabsContent value="outbound" className="pt-5 flex-1 overflow-auto">
        <OutboundTab connector={connector} onUpdate={onUpdate} />
      </TabsContent>

      <TabsContent value="metadata" className="pt-5 flex-1 overflow-auto">
        <MetadataTab connector={connector} onUpdate={onUpdate} />
      </TabsContent>
    </Tabs>
  );
}
