// =============================================================================
// ChatInput — interaction + attachment-flow tests.
//
// Covers the textarea send/keyboard paths, the file-upload happy path
// against a stubbed /api/upload, the upload-error timeout reset, the
// drag-drop pathway, and the chip remove-by-click + remove-by-keyboard
// affordances. Mirrors the same surface tested for HeroInput.
// =============================================================================

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ChatInput } from "./ChatInput";

const fetchSpy = vi.fn();

beforeEach(() => {
  fetchSpy.mockReset();
  vi.stubGlobal("fetch", fetchSpy);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function makeUploadResponse(filename = "report.md") {
  return {
    ok: true,
    statusText: "OK",
    json: async () => ({
      id: `file-${filename}`,
      filename,
      mime_type: "text/markdown",
      size: 42,
      path: `/tmp/${filename}`,
    }),
  } as unknown as Response;
}

function makeFile(name = "report.md", content = "hi") {
  return new File([content], name, { type: "text/markdown" });
}

describe("<ChatInput>", () => {
  it("does not send when text is empty even if Enter is pressed", () => {
    const onSend = vi.fn();
    render(<ChatInput onSend={onSend} disabled={false} />);
    const textarea = screen.getByPlaceholderText("Type a message...");
    fireEvent.keyDown(textarea, { key: "Enter" });
    expect(onSend).not.toHaveBeenCalled();
  });

  it("sends text on Enter and clears the input", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();
    render(<ChatInput onSend={onSend} disabled={false} />);
    const textarea = screen.getByPlaceholderText("Type a message...") as HTMLTextAreaElement;
    await user.type(textarea, "hello world");
    fireEvent.keyDown(textarea, { key: "Enter" });
    expect(onSend).toHaveBeenCalledWith("hello world", []);
    expect(textarea.value).toBe("");
  });

  it("Shift+Enter inserts a newline instead of sending", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();
    render(<ChatInput onSend={onSend} disabled={false} />);
    const textarea = screen.getByPlaceholderText("Type a message...");
    await user.type(textarea, "line one");
    fireEvent.keyDown(textarea, { key: "Enter", shiftKey: true });
    expect(onSend).not.toHaveBeenCalled();
  });

  it("ignores key-repeats so a held Enter only fires once", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();
    render(<ChatInput onSend={onSend} disabled={false} />);
    const textarea = screen.getByPlaceholderText("Type a message...");
    await user.type(textarea, "hi");
    fireEvent.keyDown(textarea, { key: "Enter", repeat: true });
    expect(onSend).not.toHaveBeenCalled();
  });

  it("Send button click submits when text is present", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();
    render(<ChatInput onSend={onSend} disabled={false} />);
    await user.type(screen.getByPlaceholderText("Type a message..."), "ok");
    fireEvent.click(screen.getByTitle("Send message"));
    expect(onSend).toHaveBeenCalledWith("ok", []);
  });

  it("Send is disabled while the disabled prop is true", () => {
    const onSend = vi.fn();
    render(<ChatInput onSend={onSend} disabled={true} />);
    const sendBtn = screen.getByTitle("Send message") as HTMLButtonElement;
    expect(sendBtn.disabled).toBe(true);
    fireEvent.click(sendBtn);
    expect(onSend).not.toHaveBeenCalled();
  });

  it("Enter while disabled is a no-op", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();
    const { rerender } = render(<ChatInput onSend={onSend} disabled={false} />);
    await user.type(screen.getByPlaceholderText("Type a message..."), "blocked");
    rerender(<ChatInput onSend={onSend} disabled={true} />);
    fireEvent.keyDown(screen.getByPlaceholderText("Type a message..."), { key: "Enter" });
    expect(onSend).not.toHaveBeenCalled();
  });

  it("uploads a selected file and shows it as a chip", async () => {
    fetchSpy.mockResolvedValueOnce(makeUploadResponse("a.md"));
    const onSend = vi.fn();
    const { container } = render(<ChatInput onSend={onSend} disabled={false} />);
    const fileInput = container.querySelector("input[type='file']") as HTMLInputElement;
    fireEvent.change(fileInput, { target: { files: [makeFile("a.md")] } });
    await waitFor(() => expect(screen.getByText("a.md")).toBeTruthy());
    expect(fetchSpy).toHaveBeenCalledWith("/api/upload", expect.objectContaining({ method: "POST" }));
  });

  it("uploads multiple files in one selection", async () => {
    fetchSpy
      .mockResolvedValueOnce(makeUploadResponse("a.md"))
      .mockResolvedValueOnce(makeUploadResponse("b.md"));
    const { container } = render(<ChatInput onSend={vi.fn()} disabled={false} />);
    const fileInput = container.querySelector("input[type='file']") as HTMLInputElement;
    fireEvent.change(fileInput, {
      target: { files: [makeFile("a.md"), makeFile("b.md")] },
    });
    await waitFor(() => {
      expect(screen.getByText("a.md")).toBeTruthy();
      expect(screen.getByText("b.md")).toBeTruthy();
    });
  });

  it("send fires onSend with attachments + clears them", async () => {
    fetchSpy.mockResolvedValueOnce(makeUploadResponse("doc.md"));
    const onSend = vi.fn();
    const user = userEvent.setup();
    const { container } = render(<ChatInput onSend={onSend} disabled={false} />);
    const fileInput = container.querySelector("input[type='file']") as HTMLInputElement;
    fireEvent.change(fileInput, { target: { files: [makeFile("doc.md")] } });
    await waitFor(() => screen.getByText("doc.md"));
    await user.type(screen.getByPlaceholderText("Type a message..."), "look");
    fireEvent.click(screen.getByTitle("Send message"));
    expect(onSend).toHaveBeenCalledWith(
      "look",
      [expect.objectContaining({ name: "doc.md", path: "/tmp/doc.md" })],
    );
    expect(screen.queryByText("doc.md")).toBeNull();
  });

  it("send works with attachments only (no text)", async () => {
    fetchSpy.mockResolvedValueOnce(makeUploadResponse("only.md"));
    const onSend = vi.fn();
    const { container } = render(<ChatInput onSend={onSend} disabled={false} />);
    fireEvent.change(container.querySelector("input[type='file']") as HTMLInputElement, {
      target: { files: [makeFile("only.md")] },
    });
    await waitFor(() => screen.getByText("only.md"));
    fireEvent.click(screen.getByTitle("Send message"));
    expect(onSend).toHaveBeenCalledWith("", expect.any(Array));
  });

  it("removes a chip when its 'x' is clicked", async () => {
    fetchSpy.mockResolvedValueOnce(makeUploadResponse("kill-me.md"));
    const { container } = render(<ChatInput onSend={vi.fn()} disabled={false} />);
    fireEvent.change(container.querySelector("input[type='file']") as HTMLInputElement, {
      target: { files: [makeFile("kill-me.md")] },
    });
    await waitFor(() => screen.getByText("kill-me.md"));
    fireEvent.click(screen.getByText("x"));
    expect(screen.queryByText("kill-me.md")).toBeNull();
  });

  it("removes a chip via Enter on the 'x' affordance", async () => {
    fetchSpy.mockResolvedValueOnce(makeUploadResponse("kbd.md"));
    const { container } = render(<ChatInput onSend={vi.fn()} disabled={false} />);
    fireEvent.change(container.querySelector("input[type='file']") as HTMLInputElement, {
      target: { files: [makeFile("kbd.md")] },
    });
    await waitFor(() => screen.getByText("kbd.md"));
    fireEvent.keyDown(screen.getByText("x"), { key: "Enter" });
    expect(screen.queryByText("kbd.md")).toBeNull();
  });

  it("removes a chip via Space on the 'x' affordance", async () => {
    fetchSpy.mockResolvedValueOnce(makeUploadResponse("space.md"));
    const { container } = render(<ChatInput onSend={vi.fn()} disabled={false} />);
    fireEvent.change(container.querySelector("input[type='file']") as HTMLInputElement, {
      target: { files: [makeFile("space.md")] },
    });
    await waitFor(() => screen.getByText("space.md"));
    fireEvent.keyDown(screen.getByText("x"), { key: " " });
    expect(screen.queryByText("space.md")).toBeNull();
  });

  it("ignores keys other than Enter/Space on the 'x' affordance", async () => {
    fetchSpy.mockResolvedValueOnce(makeUploadResponse("stay.md"));
    const { container } = render(<ChatInput onSend={vi.fn()} disabled={false} />);
    fireEvent.change(container.querySelector("input[type='file']") as HTMLInputElement, {
      target: { files: [makeFile("stay.md")] },
    });
    await waitFor(() => screen.getByText("stay.md"));
    fireEvent.keyDown(screen.getByText("x"), { key: "ArrowDown" });
    expect(screen.getByText("stay.md")).toBeTruthy();
  });

  it("shows 'Uploading...' while a file upload is in flight", async () => {
    let resolveUpload!: (r: Response) => void;
    fetchSpy.mockReturnValueOnce(new Promise<Response>((r) => { resolveUpload = r; }));
    const { container } = render(<ChatInput onSend={vi.fn()} disabled={false} />);
    fireEvent.change(container.querySelector("input[type='file']") as HTMLInputElement, {
      target: { files: [makeFile("slow.md")] },
    });
    await waitFor(() => screen.getByText("Uploading..."));
    await act(async () => {
      resolveUpload(makeUploadResponse("slow.md"));
    });
    await waitFor(() => expect(screen.queryByText("Uploading...")).toBeNull());
  });

  it("surfaces the upload error message when the response is not ok", async () => {
    fetchSpy.mockResolvedValueOnce({
      ok: false,
      statusText: "Internal Server Error",
    } as unknown as Response);
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    try {
      const { container } = render(<ChatInput onSend={vi.fn()} disabled={false} />);
      fireEvent.change(container.querySelector("input[type='file']") as HTMLInputElement, {
        target: { files: [makeFile("bad.md")] },
      });
      await waitFor(() => expect(screen.getByText(/Upload failed/)).toBeTruthy());
    } finally {
      errSpy.mockRestore();
    }
  });

  it("falls back to a generic message when the upload throws a non-Error value", async () => {
    fetchSpy.mockImplementationOnce(() => Promise.reject("string boom"));
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    try {
      const { container } = render(<ChatInput onSend={vi.fn()} disabled={false} />);
      fireEvent.change(container.querySelector("input[type='file']") as HTMLInputElement, {
        target: { files: [makeFile("nonerror.md")] },
      });
      await waitFor(() =>
        expect(
          screen.getAllByText("Upload failed").length,
        ).toBeGreaterThan(0),
      );
    } finally {
      errSpy.mockRestore();
    }
  });

  it("file-input change with no files is a no-op (no fetch, no error)", () => {
    const { container } = render(<ChatInput onSend={vi.fn()} disabled={false} />);
    fireEvent.change(container.querySelector("input[type='file']") as HTMLInputElement, {
      target: { files: [] },
    });
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("uploads via the drop handler when files are dropped on the region", async () => {
    fetchSpy.mockResolvedValueOnce(makeUploadResponse("drop.md"));
    const { container } = render(<ChatInput onSend={vi.fn()} disabled={false} />);
    const region = container.querySelector("[role='region']")!;
    // jsdom's fireEvent.drop doesn't populate `dataTransfer.files` from the
    // init dictionary the way browsers do, so we hand-roll a DragEvent with
    // a working DataTransfer-shaped property.
    const dropEvent = new Event("drop", { bubbles: true, cancelable: true }) as DragEvent;
    Object.defineProperty(dropEvent, "dataTransfer", {
      value: { files: [makeFile("drop.md")] },
    });
    const dragOverEvent = new Event("dragover", { bubbles: true, cancelable: true });
    fireEvent(region, dragOverEvent);
    fireEvent(region, dropEvent);
    await waitFor(() => expect(screen.getByText("drop.md")).toBeTruthy());
  });
});
