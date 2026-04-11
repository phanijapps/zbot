# Artifact Viewer Redesign — Design Spec

## Problem

The current artifact implementation renders artifacts inline below the agent's response in the chat view. This clutters the chat flow and makes artifacts hard to find when scrolling through a long conversation.

## Solution

Move artifacts into the **ward section of the right sidebar**. The ward section currently only shows the ward name and description — enrich it to show this session's artifacts grouped under the ward. Clicking an artifact opens a 60-65% slide-out viewer from the right.

## Design

### Ward Section Enhancement

The right sidebar's ward section changes from:

```
📁 stock-tracker
   "Stock analysis project"
```

To (when current session has artifacts in this ward):

```
📁 stock-tracker
   "Stock analysis project"
   
   📎 3 artifacts
   📄 report.html          8.7 KB
   📊 data.csv             2.1 KB
   📝 analysis.md          1.4 KB
```

- Only shows artifacts from the **current session** (not other sessions that used the same ward)
- Uses the existing `GET /api/sessions/{id}/artifacts` endpoint, filtered client-side by `ward_id`
- If session has no artifacts, the ward section stays as-is (no empty "0 artifacts" state)
- Artifact list uses same compact row style as the ArtifactsPanel (icon + label + size)

### Slide-Out Viewer

Clicking an artifact opens a slide-out panel from the right:

- Width: 60-65% of viewport
- Slides over the main content (chat stays visible behind, dimmed)
- Close button (X) in top-right corner
- Click outside or press Escape to close
- Header: file name + file type badge + file size + download button
- Body: type-specific renderer (same renderers as current ArtifactsPanel)

### What Changes

| Component | Change |
|-----------|--------|
| `ArtifactsPanel` in `AgentResponse` | **Remove** — no longer inline in chat |
| Ward section in sidebar | **Enhance** — show session artifacts |
| New: `ArtifactSlideOut` | Slide-out viewer panel |

### What Stays

- Backend: artifacts table, API endpoints, respond tool extension — all unchanged
- Transport types — unchanged  
- File type detection, MIME types — unchanged
- Inline renderers (markdown, HTML iframe, CSV table, JSON, images, video, audio, PDF, code) — moved from ArtifactsPanel to ArtifactSlideOut

## Scope

### In Scope
- Remove ArtifactsPanel from AgentResponse
- Add artifact list to ward sidebar section
- New ArtifactSlideOut component (60-65% width viewer)
- CSS for slide-out panel

### Out of Scope
- Backend changes (none needed)
- Cross-session artifact viewing
- Artifact grouping by date
- Ward file browser (showing non-artifact files)

## Files to Modify

| File | Change |
|------|--------|
| `apps/ui/src/features/chat/AgentResponse.tsx` | Remove ArtifactsPanel import and rendering |
| `apps/ui/src/features/chat/ArtifactsPanel.tsx` | Refactor: extract renderers, remove panel wrapper |
| `apps/ui/src/features/chat/ArtifactSlideOut.tsx` | New: slide-out viewer component |
| Sidebar ward component (find exact file) | Add artifact list below ward info |
| `apps/ui/src/styles/components.css` | Add slide-out CSS |
