import { expect } from "@playwright/test";
import { bootFullMode } from "../lib/harness-full";

// Measured WCAG contrast for the inspector tabs and panel text (completion
// AC4: 4.5:1 text, 3:1 necessary controls). Computes real computed styles in
// the rendered page and reports the ratios as test artifacts.
const { test, handle } = bootFullMode({fixture: "session-shell", freshVault: true, sameOrigin: true, localOnly: true});

type Rgb = [number, number, number, number]; // r, g, b, a

function parse(color: string): Rgb {
  const m = color.match(/rgba?\(([^)]+)\)/);
  if (!m) return [0, 0, 0, 1];
  const parts = m[1].split(",").map(s => parseFloat(s.trim()));
  return [parts[0], parts[1], parts[2], parts.length > 3 ? parts[3] : 1];
}

function luminance([r, g, b]: Rgb): number {
  const lin = (c: number) => (c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
  return 0.2126 * lin(r / 255) + 0.7152 * lin(g / 255) + 0.0722 * lin(b / 255);
}

function blend(fg: Rgb, bg: Rgb): Rgb {
  if (fg[3] >= 1) return fg;
  const a = fg[3];
  return [fg[0] * a + bg[0] * (1 - a), fg[1] * a + bg[1] * (1 - a), fg[2] * a + bg[2] * (1 - a), 1];
}

function ratio(fg: string, bg: string): number {
  const l1 = luminance(blend(parse(fg), parse(bg)));
  const l2 = luminance(parse(bg));
  const [hi, lo] = l1 > l2 ? [l1, l2] : [l2, l1];
  return (hi + 0.05) / (lo + 0.05);
}

test("inspector text and controls meet measured contrast floors", async ({page}) => {
  test.setTimeout(120_000);
  await page.setViewportSize({width: 1280, height: 800});
  await page.goto(handle.uiUrl("/session"));
  await page.locator("textarea").first().fill("What is 2 + 2? One-line answer.");
  await page.locator('button[title="Send message"]').click();
  await page.locator(".quick-chat__assistant").last().waitFor({timeout: 30_000});

  const measure = await page.evaluate(() => {
    const read = (el: Element) => {
      const cs = getComputedStyle(el);
      let node: Element | null = el;
      let bg = "rgb(255, 255, 255)";
      while (node) {
        const c = getComputedStyle(node).backgroundColor;
        if (c && !c.startsWith("rgba(0, 0, 0, 0)") && c !== "transparent") { bg = c; break; }
        node = node.parentElement;
      }
      return {color: cs.color, bg, outline: cs.outlineColor};
    };
    const pick = (sel: string) => {
      const el = document.querySelector(sel);
      return el ? read(el) : null;
    };
    return {
      activeTab: pick(".tab-bar__tab--active"),
      inactiveTab: pick(".tab-bar__tab:not(.tab-bar__tab--active)"),
      activityRow: pick(".session-activity__label"),
      mutedTime: pick(".session-activity__time"),
      emptyComposer: pick("textarea.chat-input__field"),
    };
  });

  const report: Record<string, number> = {};
  for (const [name, style] of Object.entries(measure)) {
    if (!style) continue;
    report[name] = ratio(style.color, style.bg);
  }
  console.log("contrast report:", JSON.stringify(report, null, 2));
  for (const [name, value] of Object.entries(report)) {
    expect(value, `${name} text contrast`).toBeGreaterThanOrEqual(4.5);
  }
});
