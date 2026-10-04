import { describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useRecentSessions } from "./useRecentSessions";

const listSessionsFull = vi.hoisted(() => vi.fn());
vi.mock("@/services/transport", () => ({ getTransport: async () => ({listSessionsFull}) }));

describe("recent sessions recovery", () => {
  it.each(["denied", "rejected"])("clears previous choices after a %s refresh", async failure => {
    listSessionsFull.mockReset();
    listSessionsFull.mockResolvedValueOnce({success:true,data:[{id:"sess-kept",title:"Earlier chat"}]});
    if (failure === "denied") listSessionsFull.mockResolvedValueOnce({success:false});
    else listSessionsFull.mockRejectedValueOnce(new Error("offline"));
    const {result,rerender} = renderHook(({key}) => useRecentSessions(key), {initialProps:{key:"first"}});
    await waitFor(() => expect(result.current.recents).toHaveLength(1));
    rerender({key:"second"});
    await waitFor(() => expect(result.current.unavailable).toBe(true));
    expect(result.current.recents).toEqual([]);
  });
});
