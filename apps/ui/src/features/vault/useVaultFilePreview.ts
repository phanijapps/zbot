import { useEffect, useRef, useState } from "react";
import { getTransport } from "@/services/transport";
import type { VaultFileResponse, VaultNode } from "@/services/transport/types";
import {
  OfficePreviewLimitError,
  parseOfficePreview,
  type OfficePreview,
} from "../chat/officePreview";

export interface SelectedVaultFileState {
  node: VaultNode;
  content: VaultFileResponse | null;
  officePreview: OfficePreview | null;
  loading: boolean;
  error: string | null;
}

export function useVaultFilePreview(wardId: string | null) {
  const [selectedFile, setSelectedFile] = useState<SelectedVaultFileState | null>(null);
  const requestRef = useRef(0);
  const wardRef = useRef(wardId);

  useEffect(() => {
    wardRef.current = wardId;
    requestRef.current += 1;
    setSelectedFile(null);
  }, [wardId]);

  async function selectFile(node: VaultNode) {
    if (!wardId) return;
    const request = requestRef.current + 1;
    requestRef.current = request;
    const initial: SelectedVaultFileState = {
      node,
      content: null,
      officePreview: null,
      loading: node.previewable,
      error: null,
    };
    setSelectedFile(initial);
    if (!node.previewable) return;

    const transport = await getTransport();
    const result = await transport.getVaultFile(wardId, node.path);
    if (wardRef.current !== wardId || requestRef.current !== request) return;
    if (!result.success || !result.data) {
      setSelectedFile({ ...initial, loading: false, error: result.error ?? "Failed to load file" });
      return;
    }

    if (result.data.kind === "office") {
      try {
        const officePreview = await parseOfficePreview(result.data.data, result.data.extension);
        if (wardRef.current !== wardId || requestRef.current !== request) return;
        setSelectedFile({
          node,
          content: result.data,
          officePreview,
          loading: false,
          error: null,
        });
      } catch (error) {
        if (wardRef.current !== wardId || requestRef.current !== request) return;
        const message = error instanceof OfficePreviewLimitError
          ? error.message
          : "Office preview failed";
        setSelectedFile({ ...initial, content: result.data, loading: false, error: message });
      }
      return;
    }

    setSelectedFile({ node, content: result.data, officePreview: null, loading: false, error: null });
  }

  function clearSelectedFile() {
    requestRef.current += 1;
    setSelectedFile(null);
  }

  return { selectedFile, selectFile, clearSelectedFile };
}
