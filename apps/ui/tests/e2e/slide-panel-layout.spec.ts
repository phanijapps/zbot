import { expect, test } from "@playwright/test";

const widths = [480, 800];

test("artifact and vault slide-outs contain long names and preview text", async ({ page }) => {
  await page.goto("/");

  for (const width of widths) {
    await page.setViewportSize({ width, height: 600 });
    for (const variant of ["artifact", "vault"] as const) {
      const bounds = await page.evaluate(({ variant }) => {
        const name = "x".repeat(220) + ".md";
        const fixture = document.createElement("div");
        fixture.innerHTML = `
          <section class="artifact-slideout ${variant === "vault" ? "vault-file-slideout" : ""}">
            <header class="artifact-slideout__header">
              <div class="artifact-slideout__title">
                <span class="artifact-slideout__icon">▣</span>
                <span>${name}</span>
                <span class="artifact-slideout__meta">${name}</span>
              </div>
              <div class="artifact-slideout__actions">
                ${variant === "artifact" ? "<button>Download</button>" : ""}
                <button>Close</button>
              </div>
            </header>
            <div class="artifact-slideout__body">
              <div class="artifact-slideout__md"><p>${"A".repeat(400)}</p></div>
            </div>
          </section>`;
        document.body.append(fixture);
        const panel = fixture.querySelector(".artifact-slideout") as HTMLElement;
        const actions = fixture.querySelector(".artifact-slideout__actions") as HTMLElement;
        const body = fixture.querySelector(".artifact-slideout__body") as HTMLElement;
        const panelRect = panel.getBoundingClientRect();
        const actionRects = [...actions.querySelectorAll("button")].map((button) => button.getBoundingClientRect());
        const result = {
          position: getComputedStyle(panel).position,
          panelLeft: panelRect.left,
          panelRight: panelRect.right,
          actionLeft: Math.min(...actionRects.map((rect) => rect.left)),
          actionRight: Math.max(...actionRects.map((rect) => rect.right)),
          bodyWidth: body.clientWidth,
          contentWidth: body.scrollWidth,
          viewport: innerWidth,
        };
        fixture.remove();
        return result;
      }, { variant });

      expect(bounds.position, `${variant} CSS is loaded`).toBe("fixed");
      expect(bounds.panelLeft, `${variant} panel at ${width}px`).toBeGreaterThanOrEqual(0);
      expect(bounds.panelRight, `${variant} panel at ${width}px`).toBeLessThanOrEqual(bounds.viewport);
      expect(bounds.actionLeft, `${variant} header actions at ${width}px`).toBeGreaterThanOrEqual(bounds.panelLeft);
      expect(bounds.actionRight, `${variant} header actions at ${width}px`).toBeLessThanOrEqual(bounds.panelRight);
      expect(bounds.contentWidth, `${variant} content at ${width}px`).toBeLessThanOrEqual(bounds.bodyWidth + 1);
    }
  }
});

test("generic slide-over stays on-screen with a long subtitle and body", async ({ page }) => {
  await page.goto("/");

  for (const width of widths) {
    await page.setViewportSize({ width, height: 600 });
    const bounds = await page.evaluate(() => {
      const fixture = document.createElement("div");
      fixture.innerHTML = `
        <aside class="slideover slideover--open">
          <header class="slideover__header">
            <div class="slideover__header-left">
              <div><div class="slideover__title">Details</div>
              <div class="slideover__subtitle">${"x".repeat(220)}</div></div>
            </div>
            <button class="slideover__close">Close</button>
          </header>
          <div class="slideover__body">${"A".repeat(400)}</div>
        </aside>`;
      document.body.append(fixture);
      const panel = fixture.querySelector(".slideover") as HTMLElement;
      const actions = fixture.querySelector(".slideover__close") as HTMLElement;
      const body = fixture.querySelector(".slideover__body") as HTMLElement;
      const panelRect = panel.getBoundingClientRect();
      const actionRect = actions.getBoundingClientRect();
      const result = {
        position: getComputedStyle(panel).position,
        panelLeft: panelRect.left,
        panelRight: panelRect.right,
        actionRight: actionRect.right,
        bodyWidth: body.clientWidth,
        contentWidth: body.scrollWidth,
        viewport: innerWidth,
      };
      fixture.remove();
      return result;
    });

    expect(bounds.position, "generic slide-over CSS is loaded").toBe("fixed");
    expect(bounds.panelLeft, `panel at ${width}px`).toBeGreaterThanOrEqual(0);
    expect(bounds.panelRight, `panel at ${width}px`).toBeLessThanOrEqual(bounds.viewport);
    expect(bounds.actionRight, `Close at ${width}px`).toBeLessThanOrEqual(bounds.panelRight);
    expect(bounds.contentWidth, `content at ${width}px`).toBeLessThanOrEqual(bounds.bodyWidth + 1);
  }
});
