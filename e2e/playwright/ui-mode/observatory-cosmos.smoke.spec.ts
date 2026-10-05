import { expect } from "@playwright/test";
import { bootFullMode } from "../lib/harness-full";

// Real-browser smoke of the GPU Observatory canvas (cosmos.gl) end-to-end:
// loads the daemon-served UI, seeds a small graph through the real store,
// and captures desktop + narrow screenshots of the visualization.
const { test, handle } = bootFullMode({fixture: "session-shell", freshVault: true, sameOrigin: true, localOnly: true});

test("observatory renders the GPU force graph with truthful chrome", async ({page, request}) => {
  test.setTimeout(180_000);
  await page.setViewportSize({width: 1280, height: 800});
  await page.goto(handle.uiUrl("/observatory"));
  // Seed a small graph through the real API, then reload to see it drawn.
  // Seed the kg sidecar directly (node:sqlite): the ingest->distillation
  // pipeline needs replay fixtures, so the smoke writes entities/edges the
  // way the store persists them, then reloads.
  const { DatabaseSync } = await import("node:sqlite");
  const { join } = await import("node:path");
  const { readdirSync, statSync } = await import("node:fs");
  const findDb = (dir: string): string => {
    for (const entry of readdirSync(dir)) {
      const full = join(dir, entry);
      if (statSync(full).isDirectory()) {
        const nested = findDb(full);
        if (nested) return nested;
      } else if (entry === "engram_data.db") {
        return full;
      }
    }
    return "";
  };
  const dbPath = findDb(handle.dataDir());
  const db = new DatabaseSync(dbPath);
  const now = "2026-10-05T00:00:00Z";
  const types = ["concept", "tool", "person", "organization", "location"];
  const count = 48;
  for (let index = 0; index < count; index += 1) {
    const id = `smoke-${index}`;
    const entityType = types[index % types.length];
    const properties = JSON.stringify({note: `seed ${index}`});
    const entityJson = JSON.stringify({
      id, agent_id: "root", entity_type: entityType, name: `Seeded ${entityType} ${index}`,
      properties: {note: `seed ${index}`}, mention_count: 1 + (index % 9),
      first_seen_at: now, last_seen_at: now,
    });
    db.prepare(
      "INSERT OR REPLACE INTO kg_entities (id, agent_id, entity_type, name, first_seen_at, last_seen_at, mention_count, properties_json, entity_json, archived, pruned, layer) VALUES (?, 'root', ?, ?, ?, ?, ?, ?, ?, 0, 0, 0)"
    ).run(id, entityType, `Seeded ${entityType} ${index}`, now, now, 1 + (index % 9), properties, entityJson);
  }
  for (let index = 0; index < 60; index += 1) {
    const source = `smoke-${index % count}`;
    const target = `smoke-${(index * 7 + 3) % count}`;
    const id = `rel-smoke-${index}`;
    const relationshipJson = JSON.stringify({
      id, agent_id: "root", source_entity_id: source, target_entity_id: target,
      relationship_type: "related_to", properties: {}, mention_count: 1,
      first_seen_at: now, last_seen_at: now,
    });
    db.prepare(
      "INSERT OR REPLACE INTO kg_relationships (id, agent_id, source_entity_id, target_entity_id, relationship_type, first_seen_at, last_seen_at, mention_count, properties_json, relationship_json, archived) VALUES (?, 'root', ?, ?, 'related_to', ?, ?, 1, '{}', ?, 0)"
    ).run(id, source, target, now, now, relationshipJson);
  }
  db.close();
  await page.reload();
  await expect(page.locator(".observatory__cosmos-host canvas")).toBeVisible({timeout: 30_000});
  await expect(page.locator(".observatory__legend")).toBeVisible();
  await expect(page.getByRole("status")).toBeVisible();
  await page.screenshot({path: "test-results/observatory-cosmos-1280.png"});
  await page.setViewportSize({width: 390, height: 844});
  await page.screenshot({path: "test-results/observatory-cosmos-390.png"});
});
