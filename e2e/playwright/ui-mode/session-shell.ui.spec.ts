import { expect } from "@playwright/test";
import { bootFullMode } from "../lib/harness-full";

// Use the real loopback daemon + served UI: separate-origin preview would
// bypass the desktop boundary if tests rewrote Origin to make it pass.
// Local-only is explicit: this suite does not assert the unresolved default bind.
const { test, handle } = bootFullMode({fixture: "session-shell", freshVault: true, sameOrigin: true, localOnly: true});

test("shell and legacy Chat/Research preserve final answers on reload", async ({page, request}) => {
  test.setTimeout(180_000);
  await page.setViewportSize({width: 1280, height: 800});
  let reservedHistory = "";
  let independentCreates = 0;
  page.on("request", req => { if (req.method() === "POST" && new URL(req.url()).pathname === "/api/sessions/chat") independentCreates++; });
  for (const path of ["/chat", "/session", "/research", "/session?mode=research"]) {
    await page.goto(handle.uiUrl(path));
    const answers = page.locator(path === "/chat" || path === "/session" ? ".quick-chat__assistant" : ".research-msg--assistant");
    if (path === "/session") {
      await expect(page.locator(".quick-chat__messages")).toHaveText(reservedHistory, {useInnerText:true});
      expect(independentCreates).toBe(0);
    }
    await expect(page.locator("textarea").first()).toBeEnabled();
    const previousAnswers = await answers.count();
    await page.locator("textarea").first().fill("What is 2 + 2? One-line answer.");
    await page.locator('button[title="Send message"]').click();
    await expect(answers).toHaveCount(previousAnswers + 1, {timeout:30_000});
    const answer = answers.last();
    await expect(answer).toContainText("4", {timeout:30_000});
    await expect(page.getByRole("button", {name: /Stop (chat|research)/i})).toHaveCount(0, {timeout:10_000});
    const reopen = new URL(page.url()).pathname;
    if (path === "/session") {
      expect(reopen).toBe("/session");
      expect(independentCreates).toBe(0);
    } else if (path.startsWith("/session")) {
      expect(reopen).toMatch(/^\/session\/sess-/);
      const id = reopen.split("/").pop()!;
      const persisted = await request.get(handle.gatewayUrl(`/api/executions/v2/sessions/${id}/full`));
      expect(persisted.ok()).toBeTruthy();
      expect((await persisted.json()).mode).toBe(path.includes("research") ? "research" : "fast");
    }
    await page.goto(handle.uiUrl(reopen));
    await expect(answer).toContainText("4", {timeout:20_000});
    if (path === "/chat" || path === "/session") reservedHistory = await page.locator(".quick-chat__messages").innerText();
  }
  await handle.assertZeroDrift(request);
  const conversation = new URL(page.url()).pathname;
  await page.getByRole("tab", {name:"Chat",exact:true}).click();
  await expect(page).toHaveURL(url => url.pathname === "/session" && !url.search);
  await expect(page.locator(".quick-chat__messages")).toHaveText(reservedHistory, {useInnerText:true});
  expect(independentCreates).toBe(0);
  await page.getByRole("button", {name:"New chat",exact:true}).click();
  await expect(page).toHaveURL(/\/session\/sess-/);
  const independentId = new URL(page.url()).pathname;
  expect(independentCreates).toBe(1);
  expect(independentId).not.toBe(conversation);
  await expect(page.locator(".quick-chat__assistant")).toHaveCount(0);
  await page.getByRole("tab", {name:"Chat",exact:true}).click();
  await expect(page.locator(".quick-chat__messages")).toHaveText(reservedHistory, {useInnerText:true});
  expect(independentCreates).toBe(1);
  await page.goto(handle.uiUrl(conversation));
  await expect(page.locator(".research-msg--assistant").first()).toContainText("4");
  for (const name of ["Memory", "Observatory"]) {
    await page.getByRole("link", {name, exact:true}).click();
    await expect(page.getByRole("heading", {name, exact:true, level:1})).toBeVisible();
    if (name === "Memory") {
      await page.getByRole("button", {name:/__global__/}).first().click();
      const graph = page.getByRole("link", {name:/Graph/});
      await expect(graph).toHaveAttribute("href", `/observatory?returnTo=${encodeURIComponent(conversation)}`);
      const popupPromise = page.context().waitForEvent("page");
      await graph.click();
      const popup = await popupPromise;
      await expect(popup.getByRole("heading", {name:"Observatory", exact:true})).toBeVisible();
      await popup.reload();
      await popup.getByRole("link", {name:"Back to conversation"}).click();
      await expect(popup).toHaveURL(url => url.pathname === conversation);
      await expect(popup.locator(".research-msg--assistant").first()).toContainText("4");
      await popup.close();
    }
    await page.reload();
    await page.getByRole("link", {name:"Back to conversation"}).click();
    await expect(page).toHaveURL(url => url.pathname === conversation);
    await expect(page.locator(".research-msg--assistant").first()).toContainText("4");
  }
  await expect(page.getByRole("link", {name:"Memory"})).toBeVisible();
  await expect(page.getByRole("link", {name:"Observatory"})).toBeVisible();
  await expect(page.getByRole("link", {name:/ward explorer/i})).toHaveCount(0);
  await page.screenshot({path:"../../docs/specs/desktop-session-shell/desktop.png", fullPage:true});
  await page.setViewportSize({width:720,height:800});
  await expect(page.getByRole("tab", {name:"Research",exact:true})).toBeVisible();
  await page.getByRole("button", {name:"Toggle session details"}).click();
  await expect(page.getByRole("tab", {name:"Files",exact:true})).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
  await page.screenshot({path:"../../docs/specs/desktop-session-shell/narrow.png", fullPage:true});
});

// Seeded presentation contract: these route responses are not live execution
// evidence. The preceding test exercises the real daemon and providers.
test("knowledge layouts and all subagent states are readable at each shell band", async ({page}, testInfo) => {
  test.setTimeout(180_000);
  const id = "sess-visual";
  const time = "2026-04-19T00:00:00.000Z";
  const row = {conversation_id:id, agent_id:"root", agent_name:"root", started_at:time,
    ended_at:"2026-04-19T00:01:00.000Z", status:"completed", token_count:0,
    tool_call_count:0, error_count:0, child_session_ids:[], title:"Contrast review"};
  const statuses = ["running", "completed", "stopped", "error"];
  const rows = [{...row, session_id:"exec-root"}, ...statuses.map((status,i) => ({...row,
    session_id:`exec-child-${i}`, parent_session_id:"exec-root", agent_id:`reviewer-${status}`,
    status, ended_at:status === "running" ? null : row.ended_at}))];
  const messages = [{id:"msg-user", execution_id:"exec-root", agent_id:"root", delegation_type:"root",
    role:"user", content:"Review the evidence and explain what changed.", created_at:time},
    {id:"msg-answer", execution_id:"exec-root", agent_id:"root", delegation_type:"root",
      role:"assistant", content:"Review complete. Expand each agent to inspect its result.", created_at:row.ended_at},
    ...statuses.map((status,i) => ({id:`msg-child-${i}`, execution_id:`exec-child-${i}`,
      agent_id:`reviewer-${status}`, delegation_type:"delegate", role:"assistant",
      content:"Readable evidence with a long reference: "+"long-reference-".repeat(12), created_at:time}))];
  await page.route(`**/api/executions/v2/sessions/${id}/full`, route => route.fulfill({json:{id, mode:"research", executions:[]}}));
  await page.route("**/api/logs/sessions?**", route => route.fulfill({json:rows}));
  await page.route(`**/api/executions/v2/sessions/${id}/messages?**`, route => route.fulfill({json:messages}));
  await page.route("**/api/graph/all/entities?**", route => route.fulfill({json:{entities:[
    {id:"visual-entity", name:"Desktop agent", entity_type:"concept", mention_count:1, agent_id:"root", description:"Seeded knowledge for layout inspection", created_at:time, updated_at:time}
  ], total:1}}));
  await page.route("**/api/graph/all/relationships?**", route => route.fulfill({json:{relationships:[],total:0}}));
  // Presentation fixtures must not depend on a preceding execution test
  // creating global memory during its successful Research journey.
  await page.route("**/api/wards", route => route.fulfill({json:[{id:"__global__",count:1}]}));
  await page.route("**/api/wards/__global__/content", route => route.fulfill({json:{
    ward_id:"__global__",summary:{description:"Seeded memory"},counts:{facts:1,wiki:0,procedures:0,episodes:0},
    facts:[{id:"visual-fact",content:"Keep the desktop conversation readable.",category:"preference",confidence:1,created_at:time,age_bucket:"today",ward_id:"__global__"}],
    wiki:[],procedures:[],episodes:[]
  }}));
  const records: unknown[] = [];
  for (const path of [`/session/${id}`, "/memory", "/observatory"]) {
    for (const width of [320,760,1100]) for (const height of [600,900]) {
      await page.setViewportSize({width,height});
      await page.goto(handle.uiUrl(path));
      await expect(page.locator(".session-shell")).toBeVisible();
      if (path.startsWith("/session")) {
        await expect(page.locator(".subagent-card")).toHaveCount(4);
        for (const card of await page.locator(".subagent-card").all()) {
          if (await card.locator(".subagent-card__toggle").getAttribute("aria-expanded") === "false") await card.locator(".subagent-card__toggle").click();
        }
        const contrasts = await page.locator(".subagent-card").evaluateAll(cards => {
          const rgb = (value:string) => value.match(/[\d.]+/g)!.slice(0,3).map(Number);
          const luminance = (value:string) => rgb(value).map(n => { const v=n/255; return v<=.04045?v/12.92:Math.pow((v+.055)/1.055,2.4); }).reduce((sum,n,i) => sum+n*[.2126,.7152,.0722][i],0);
          const contrast = (a:string,b:string) => {const x=luminance(a),y=luminance(b);return (Math.max(x,y)+.05)/(Math.min(x,y)+.05);};
          return cards.map(card => {
            const style=getComputedStyle(card);
            return {status:card.getAttribute("data-status"), text:contrast(style.color,style.backgroundColor),
              label:contrast(getComputedStyle(card.querySelector(".subagent-card__state")!).color,style.backgroundColor),
              border:contrast(style.borderLeftColor,style.backgroundColor)};
          });
        });
        for (const item of contrasts) { expect(item.text).toBeGreaterThanOrEqual(4.5); expect(item.label).toBeGreaterThanOrEqual(4.5); expect(item.border).toBeGreaterThanOrEqual(3); }
        records.push({path,width,height,contrasts});
      } else if (path === "/memory") {
        await expect(page.getByRole("region", {name:"Memory command deck"})).toBeVisible();
        await page.getByRole("button", {name:/__global__/}).first().click();
        await expect(page.locator(".memory-item").first()).toBeVisible();
        const activeContrasts = await page.locator(".memory-search__mode button.is-active, .memory-chip.is-on, .memory-ward.is-active, .memory-write__btn").evaluateAll(controls => {
          const luminance = (value:string) => value.match(/[\d.]+/g)!.slice(0,3).map(Number).map(n => {
            const v=n/255; return v<=.04045?v/12.92:Math.pow((v+.055)/1.055,2.4);
          }).reduce((sum,n,i) => sum+n*[.2126,.7152,.0722][i],0);
          return controls.map(el => { const style=getComputedStyle(el),fg=luminance(style.color),bg=luminance(style.backgroundColor);
            return {control:el.className,contrast:(Math.max(fg,bg)+.05)/(Math.min(fg,bg)+.05)};
          });
        });
        for (const item of activeContrasts) expect(item.contrast).toBeGreaterThanOrEqual(4.5);
        records.push({path,width,height,activeContrasts});
      } else await expect(page.locator(".graph-node").first()).toBeVisible({timeout: 20_000});
      if (width === 1100 && height === 900 && process.env.DESKTOP_AXE_SCRIPT) {
        await page.addScriptTag({path:process.env.DESKTOP_AXE_SCRIPT});
        const audit = await page.evaluate(async () => {
          const runner = (window as unknown as {axe:{run(context:Document, options:unknown):Promise<{violations:unknown[]}>}}).axe;
          return runner.run(document, {runOnly:{type:"tag",values:["wcag2a","wcag2aa","wcag21aa"]}});
        });
        records.push({path,a11y:audit.violations});
        expect(audit.violations).toEqual([]);
      }
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
      const region = page.locator(path.startsWith("/session") ? ".session-shell__scroll" : ".session-shell__knowledge-content");
      for (const position of ["rest", "scrolled"]) {
        const record = await region.evaluate((host, position) => {
          const candidates = [host, ...host.querySelectorAll(".memory-tab-deck, .memory-deck__body")];
          const el = candidates.find(candidate => candidate.scrollHeight > candidate.clientHeight && ["auto","scroll"].includes(getComputedStyle(candidate).overflowY)) ?? host;
          el.scrollTop = position === "rest" ? 0 : el.scrollHeight;
          return {path:location.pathname,width:innerWidth,height:innerHeight,offset:scrollY,
            scrollable:document.documentElement.scrollHeight>innerHeight,region:el.className,
            regionOffset:el.scrollTop,regionScrollable:el.scrollHeight>el.clientHeight};
        },position);
        records.push(record);
        await page.screenshot({path:testInfo.outputPath(`${path.split("/")[1]}-${width}-${height}-${position}.png`)});
        if (!record.regionScrollable) break;
      }
    }
  }
  await testInfo.attach("layout-and-contrast", {body:JSON.stringify(records,null,2), contentType:"application/json"});
});

test("reserved Quick Chat fits the shell with history and empty state", async ({page}, testInfo) => {
  const id = "sess-quick-visual";
  let populated = true;
  await page.route("**/api/chat/init", route => route.fulfill({json:{sessionId:id,conversationId:"chat-quick-visual",created:false}}));
  await page.route(`**/api/executions/v2/sessions/${id}/messages?**`, route => route.fulfill({json:populated ? [
    {id:"quick-question",agent_id:"root",role:"user",content:"Continue our existing Quick Chat.",created_at:"2026-09-27T12:00:00Z"},
    {id:"quick-answer",agent_id:"root",role:"assistant",content:Array.from({length:20}, (_,i) => `Evidence ${i+1}: preserved conversation history remains readable inside the desktop shell, with its composer and mode controls available.`).join("\n\n"),created_at:"2026-09-27T12:00:01Z"}
  ] : []}));
  const records: unknown[] = [];
  for (const state of ["history","empty"]) {
    populated = state === "history";
    for (const width of [320,760,1100]) for (const height of [600,900]) {
      await page.setViewportSize({width,height});
      await page.goto(handle.uiUrl("/session"));
      await expect(page.locator("textarea")).toBeEnabled();
      if (populated) await expect(page.locator(".quick-chat__assistant")).toContainText("Evidence 20");
      else await expect(page.getByRole("heading", {name:"Quick chat",exact:true})).toBeVisible();
      await expect(page.getByRole("tab", {name:"Chat",exact:true})).toBeVisible();
      await expect(page.locator("textarea")).toBeInViewport();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
      if (width === 1100 && height === 900 && process.env.DESKTOP_AXE_SCRIPT) {
        await page.addScriptTag({path:process.env.DESKTOP_AXE_SCRIPT});
        const audit = await page.evaluate(async () => (window as unknown as {axe:{run(context:Document,options:unknown):Promise<{violations:unknown[]}>}}).axe.run(document,{runOnly:{type:"tag",values:["wcag2a","wcag2aa","wcag21aa"]}}));
        records.push({state,a11y:audit.violations});
        expect(audit.violations).toEqual([]);
      }
      const region = page.locator(populated ? ".quick-chat__scroll" : ".quick-chat__empty");
      for (const position of ["rest","scrolled"]) {
        const record = await region.evaluate((el,position) => {
          el.scrollTop = position === "rest" ? 0 : el.scrollHeight;
          return {route:location.pathname,width:innerWidth,height:innerHeight,offset:scrollY,
            scrollable:document.documentElement.scrollHeight>innerHeight,region:el.className,
            regionOffset:el.scrollTop,regionScrollable:el.scrollHeight>el.clientHeight};
        },position);
        records.push({state,...record});
        await page.screenshot({path:testInfo.outputPath(`quick-${state}-${width}-${height}-${position}.png`)});
        if (!record.regionScrollable) break;
      }
    }
  }
  await testInfo.attach("quick-chat-layout", {body:JSON.stringify(records,null,2),contentType:"application/json"});
});
