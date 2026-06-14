import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type MouseEvent as ReactMouseEvent,
} from "react";
import { useSearchParams } from "react-router-dom";
import {
  Archive,
  ChevronRight,
  Code2,
  File,
  FileText,
  Folder,
  FolderOpen,
  PanelLeftClose,
  PanelLeftOpen,
  Presentation,
  RefreshCw,
  Search,
  X,
} from "lucide-react";
import { getTransport } from "@/services/transport";
import type {
  VaultFileResponse,
  VaultNode,
  VaultOfficeFileResponse,
  VaultTextFileResponse,
  VaultWard,
} from "@/services/transport/types";
import { Markdown } from "../shared/markdown";
import {
  OfficePreviewLimitError,
  parseOfficePreview,
  type OfficePreview,
} from "../chat/officePreview";

type LoadState = "idle" | "loading" | "error";

const MIN_EXPLORER_WIDTH = 240;
const DEFAULT_EXPLORER_WIDTH = 340;
const MAX_EXPLORER_WIDTH = 560;
const EXPLORER_WIDTH_STEP = 24;

function clampExplorerWidth(width: number, splitElement: HTMLDivElement | null) {
  const splitWidth = splitElement?.getBoundingClientRect().width ?? 0;
  const availableMax = splitWidth > 0
    ? Math.max(MIN_EXPLORER_WIDTH, Math.min(MAX_EXPLORER_WIDTH, splitWidth - 420))
    : MAX_EXPLORER_WIDTH;
  return Math.min(availableMax, Math.max(MIN_EXPLORER_WIDTH, Math.round(width)));
}

interface SelectedFileState {
  node: VaultNode;
  content: VaultFileResponse | null;
  officePreview: OfficePreview | null;
  loading: boolean;
  error: string | null;
}

export function VaultPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const requestedWardId = searchParams.get("ward");
  const activeSection = searchParams.get("section");
  const showingWards = activeSection === "wards";
  const [wards, setWards] = useState<VaultWard[]>([]);
  const [wardState, setWardState] = useState<LoadState>("loading");
  const [wardError, setWardError] = useState<string | null>(null);
  const [selectedWard, setSelectedWard] = useState<VaultWard | null>(null);
  const [childrenByPath, setChildrenByPath] = useState<Record<string, VaultNode[]>>({});
  const [truncatedByPath, setTruncatedByPath] = useState<Record<string, boolean>>({});
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(new Set());
  const [treeError, setTreeError] = useState<string | null>(null);
  const [selectedFile, setSelectedFile] = useState<SelectedFileState | null>(null);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [explorerWidth, setExplorerWidth] = useState(DEFAULT_EXPLORER_WIDTH);
  const [isResizingExplorer, setIsResizingExplorer] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchState, setSearchState] = useState<LoadState>("idle");
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searchResults, setSearchResults] = useState<VaultNode[]>([]);
  const [searchTruncated, setSearchTruncated] = useState(false);
  const treeGenerationRef = useRef(0);
  const fileRequestRef = useRef(0);
  const searchRequestRef = useRef(0);
  const splitRef = useRef<HTMLDivElement | null>(null);
  const resizeStartRef = useRef({ clientX: 0, width: DEFAULT_EXPLORER_WIDTH });

  useEffect(() => {
    void loadWards();
  }, []);

  useEffect(() => {
    if (!isResizingExplorer) return;

    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    function handleMouseMove(event: MouseEvent) {
      const delta = event.clientX - resizeStartRef.current.clientX;
      setExplorerWidth(clampExplorerWidth(resizeStartRef.current.width + delta, splitRef.current));
    }

    function stopResizing() {
      setIsResizingExplorer(false);
    }

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", stopResizing);
    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", stopResizing);
    };
  }, [isResizingExplorer]);

  useEffect(() => {
    if (wardState !== "idle") return;
    if (!requestedWardId) {
      if (selectedWard) resetWardExplorer();
      return;
    }
    if (selectedWard?.id === requestedWardId) return;

    const ward = wards.find((item) => item.id === requestedWardId);
    if (ward) {
      void activateWard(ward);
    } else {
      resetWardExplorer();
    }
  }, [requestedWardId, selectedWard?.id, wardState, wards]);

  useEffect(() => {
    const query = searchQuery.trim();
    searchRequestRef.current += 1;
    const request = searchRequestRef.current;
    if (!selectedWard || !query) {
      setSearchState("idle");
      setSearchError(null);
      setSearchResults([]);
      setSearchTruncated(false);
      return;
    }

    setSearchState("loading");
    setSearchError(null);
    setSearchResults([]);
    setSearchTruncated(false);
    const timer = window.setTimeout(() => {
      void searchVaultFiles(selectedWard.id, query, request);
    }, 180);
    return () => window.clearTimeout(timer);
  }, [searchQuery, selectedWard?.id]);

  async function loadWards() {
    setWardState("loading");
    setWardError(null);
    const transport = await getTransport();
    const result = await transport.listVaultWards();
    if (!result.success || !result.data) {
      setWardState("error");
      setWardError(result.error ?? "Failed to load wards");
      return;
    }
    setWards(result.data.wards);
    setWardState("idle");
  }

  async function activateWard(ward: VaultWard) {
    const generation = treeGenerationRef.current + 1;
    treeGenerationRef.current = generation;
    fileRequestRef.current += 1;
    searchRequestRef.current += 1;
    setSelectedWard(ward);
    setSelectedFile(null);
    setSearchQuery("");
    setSearchState("idle");
    setSearchError(null);
    setSearchResults([]);
    setSearchTruncated(false);
    setChildrenByPath({});
    setTruncatedByPath({});
    setExpanded(new Set([""]));
    await loadTree(ward.id, "", generation);
  }

  function selectWard(ward: VaultWard) {
    setSearchParams({ ward: ward.id });
  }

  function openWards() {
    resetWardExplorer();
    setSearchParams({ section: "wards" });
  }

  async function loadTree(wardId: string, path: string, generation: number) {
    setTreeError(null);
    setLoadingPaths((current) => new Set(current).add(path));
    const transport = await getTransport();
    const result = await transport.getVaultTree(wardId, path);
    if (treeGenerationRef.current !== generation) return;
    setLoadingPaths((current) => {
      const next = new Set(current);
      next.delete(path);
      return next;
    });
    if (!result.success || !result.data) {
      setTreeError(result.error ?? "Failed to load directory");
      return;
    }
    const data = result.data;
    setChildrenByPath((current) => ({ ...current, [path]: data.children }));
    setTruncatedByPath((current) => ({ ...current, [path]: data.truncated }));
  }

  async function toggleDirectory(node: VaultNode) {
    if (!selectedWard) return;
    const isOpen = expanded.has(node.path);
    setExpanded((current) => {
      const next = new Set(current);
      if (isOpen) next.delete(node.path);
      else next.add(node.path);
      return next;
    });
    if (!isOpen && childrenByPath[node.path] == null) {
      await loadTree(selectedWard.id, node.path, treeGenerationRef.current);
    }
  }

  async function searchVaultFiles(wardId: string, query: string, request: number) {
    const transport = await getTransport();
    const result = await transport.searchVaultFiles(wardId, query, 30);
    if (searchRequestRef.current !== request || selectedWard?.id !== wardId) return;
    if (!result.success || !result.data) {
      setSearchState("error");
      setSearchError(result.error ?? "Failed to search files");
      setSearchResults([]);
      setSearchTruncated(false);
      return;
    }
    setSearchState("idle");
    setSearchError(null);
    setSearchResults(result.data.matches);
    setSearchTruncated(result.data.truncated);
  }

  async function selectFile(node: VaultNode) {
    if (!selectedWard) return;
    const generation = treeGenerationRef.current;
    const fileRequest = fileRequestRef.current + 1;
    fileRequestRef.current = fileRequest;
    const initial: SelectedFileState = {
      node,
      content: null,
      officePreview: null,
      loading: node.previewable,
      error: null,
    };
    setSelectedFile(initial);
    if (!node.previewable) return;

    const transport = await getTransport();
    const result = await transport.getVaultFile(selectedWard.id, node.path);
    if (treeGenerationRef.current !== generation || fileRequestRef.current !== fileRequest) return;
    if (!result.success || !result.data) {
      setSelectedFile({ ...initial, loading: false, error: result.error ?? "Failed to load file" });
      return;
    }

    if (result.data.kind === "office") {
      try {
        const officePreview = await parseOfficePreview(result.data.data, result.data.extension);
        if (treeGenerationRef.current !== generation || fileRequestRef.current !== fileRequest) return;
        setSelectedFile({
          node,
          content: result.data,
          officePreview,
          loading: false,
          error: null,
        });
      } catch (error) {
        if (treeGenerationRef.current !== generation || fileRequestRef.current !== fileRequest) return;
        const message = error instanceof OfficePreviewLimitError
          ? error.message
          : "Office preview failed";
        setSelectedFile({ ...initial, content: result.data, loading: false, error: message });
      }
      return;
    }

    setSelectedFile({ node, content: result.data, officePreview: null, loading: false, error: null });
  }

  async function openSelectedWardFolder() {
    if (!selectedWard) return;
    const transport = await getTransport();
    await transport.openWard(selectedWard.id);
  }

  function resetWardExplorer() {
    setSelectedWard(null);
    setSelectedFile(null);
    treeGenerationRef.current += 1;
    fileRequestRef.current += 1;
    searchRequestRef.current += 1;
    setSearchQuery("");
    setSearchState("idle");
    setSearchError(null);
    setSearchResults([]);
    setSearchTruncated(false);
    setChildrenByPath({});
    setTruncatedByPath({});
    setExpanded(new Set());
    setLoadingPaths(new Set());
    setTreeError(null);
  }

  function returnToWards() {
    openWards();
  }

  function returnToVaultRoot() {
    resetWardExplorer();
    setSearchParams({});
  }

  function beginExplorerResize(event: ReactMouseEvent<HTMLDivElement>) {
    event.preventDefault();
    resizeStartRef.current = { clientX: event.clientX, width: explorerWidth };
    setIsResizingExplorer(true);
  }

  function resizeExplorerBy(delta: number) {
    setExplorerWidth((current) => clampExplorerWidth(current + delta, splitRef.current));
  }

  function handleExplorerResizeKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      resizeExplorerBy(-EXPLORER_WIDTH_STEP);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      resizeExplorerBy(EXPLORER_WIDTH_STEP);
    } else if (event.key === "Home") {
      event.preventDefault();
      setExplorerWidth(MIN_EXPLORER_WIDTH);
    } else if (event.key === "End") {
      event.preventDefault();
      setExplorerWidth(clampExplorerWidth(MAX_EXPLORER_WIDTH, splitRef.current));
    }
  }

  const breadcrumb = useMemo(() => (
    selectedWard
      ? ["Vault", "Wards", selectedWard.name]
      : showingWards
        ? ["Vault", "Wards"]
        : ["Vault"]
  ), [selectedWard, showingWards]);
  const wardsExpanded = showingWards || Boolean(selectedWard);
  const rootChildren = childrenByPath[""] ?? [];
  const visibleDirectoryCount = rootChildren.filter((child) => child.kind === "directory").length;
  const visibleFileCount = rootChildren.filter((child) => child.kind === "file").length;
  const headerStatus = selectedWard
    ? `${visibleDirectoryCount} dirs / ${visibleFileCount} files`
    : showingWards
      ? `${wards.length} wards`
      : "Vault root";

  return (
    <div className="vault-page">
      <header className="vault-page__header">
        <div className="vault-page__identity">
          <span className="vault-page__mark" aria-hidden="true">
            <Archive size={18} />
          </span>
          <nav className="vault-breadcrumb" aria-label="Breadcrumb">
            {breadcrumb.map((item, index) => (
              <span className="vault-breadcrumb__item" key={`${item}-${index}`}>
                {index > 0 ? <ChevronRight size={14} aria-hidden="true" /> : null}
                {(selectedWard || showingWards) && index === 0 ? (
                  <button
                    className="vault-breadcrumb__button"
                    type="button"
                    onClick={returnToVaultRoot}
                  >
                    {item}
                  </button>
                ) : selectedWard && index === 1 ? (
                  <button
                    className="vault-breadcrumb__button"
                    type="button"
                    onClick={returnToWards}
                  >
                    {item}
                  </button>
                ) : (
                  <span>{item}</span>
                )}
              </span>
            ))}
          </nav>
        </div>
        <div className="vault-page__actions">
          <span className="vault-status-pill">{headerStatus}</span>
          <button
            className="icon-btn"
            type="button"
            onClick={() => setSidebarCollapsed((current) => !current)}
            aria-label={sidebarCollapsed ? "Expand vault explorer" : "Collapse vault explorer"}
            aria-pressed={sidebarCollapsed}
          >
            {sidebarCollapsed ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
          </button>
          <button className="icon-btn" type="button" onClick={loadWards} aria-label="Refresh vault wards">
            <RefreshCw size={16} />
          </button>
        </div>
      </header>

      <div
        ref={splitRef}
        className={`vault-split${sidebarCollapsed ? " vault-split--collapsed" : ""}${isResizingExplorer ? " vault-split--resizing" : ""}`}
        style={{ "--vault-explorer-width": `${explorerWidth}px` } as CSSProperties}
      >
        {!sidebarCollapsed ? (
          <>
            <aside className="vault-tree-pane" aria-label="Vault explorer">
              <div className="vault-tree-pane__head">
                <span className="vault-tree-pane__title">{selectedWard ? selectedWard.name : "Vault"}</span>
                <div className="vault-tree-pane__actions">
                  <span className="vault-tree-pane__meta">
                    {selectedWard ? "Ward content" : wardState === "loading" ? "Loading" : `${wards.length} wards`}
                  </span>
                  <button
                    className="icon-btn icon-btn--sm"
                    type="button"
                    onClick={() => setSidebarCollapsed(true)}
                    aria-label="Collapse vault explorer"
                  >
                    <PanelLeftClose size={14} />
                  </button>
                </div>
              </div>
              {treeError ? <p className="vault-state vault-state--error">{treeError}</p> : null}
              {selectedWard ? (
                <VaultSearchBox
                  wardName={selectedWard.name}
                  query={searchQuery}
                  state={searchState}
                  error={searchError}
                  results={searchResults}
                  truncated={searchTruncated}
                  selectedPath={selectedFile?.node.path ?? null}
                  onQueryChange={setSearchQuery}
                  onClear={() => setSearchQuery("")}
                  onSelectFile={(node) => void selectFile(node)}
                />
              ) : null}
              <VaultExplorer
                wards={wards}
                state={wardState}
                error={wardError}
                wardsExpanded={wardsExpanded}
                selectedWard={selectedWard}
                wardChildren={childrenByPath[""] ?? []}
                childrenByPath={childrenByPath}
                truncatedByPath={truncatedByPath}
                expanded={expanded}
                loadingPaths={loadingPaths}
                selectedPath={selectedFile?.node.path ?? null}
                onOpenWards={openWards}
                onSelectWard={selectWard}
                onToggleDirectory={(node) => void toggleDirectory(node)}
                onSelectFile={(node) => void selectFile(node)}
              />
            </aside>
            <div
              className="vault-split__resizer"
              role="separator"
              aria-label="Resize vault explorer"
              aria-orientation="vertical"
              aria-valuemin={MIN_EXPLORER_WIDTH}
              aria-valuemax={MAX_EXPLORER_WIDTH}
              aria-valuenow={explorerWidth}
              tabIndex={0}
              onMouseDown={beginExplorerResize}
              onKeyDown={handleExplorerResizeKeyDown}
            >
              <span aria-hidden="true" />
            </div>
          </>
        ) : null}
        {selectedWard ? (
          <FilePreviewPane
            selected={selectedFile}
            ward={selectedWard}
            directoryCount={visibleDirectoryCount}
            fileCount={visibleFileCount}
            onOpenWard={openSelectedWardFolder}
          />
        ) : (
          <section className="vault-preview-pane" aria-label="Vault file preview">
            <EmptyPreview showingWards={showingWards} wardCount={wards.length} />
          </section>
        )}
      </div>
    </div>
  );
}

function EmptyPreview({
  showingWards,
  wardCount,
}: {
  showingWards: boolean;
  wardCount: number;
}) {
  return (
    <div className="vault-empty-preview">
      <div className="vault-empty-preview__glyph" aria-hidden="true">
        <Archive size={34} />
      </div>
      <div>
        <p className="vault-empty-preview__eyebrow">{showingWards ? `${wardCount} wards` : "Vault root"}</p>
        <p className="vault-state">No file selected.</p>
      </div>
    </div>
  );
}

function VaultSearchBox({
  wardName,
  query,
  state,
  error,
  results,
  truncated,
  selectedPath,
  onQueryChange,
  onClear,
  onSelectFile,
}: {
  wardName: string;
  query: string;
  state: LoadState;
  error: string | null;
  results: VaultNode[];
  truncated: boolean;
  selectedPath: string | null;
  onQueryChange: (query: string) => void;
  onClear: () => void;
  onSelectFile: (node: VaultNode) => void;
}) {
  const hasQuery = query.trim().length > 0;
  return (
    <section className="vault-search" aria-label="Vault file search">
      <div className="vault-search__field">
        <Search size={14} aria-hidden="true" />
        <input
          type="search"
          value={query}
          onChange={(event) => onQueryChange(event.currentTarget.value)}
          placeholder={`Search ${wardName}`}
          aria-label={`Fuzzy search files in ${wardName}`}
        />
        {hasQuery ? (
          <button className="vault-search__clear" type="button" onClick={onClear} aria-label="Clear file search">
            <X size={14} />
          </button>
        ) : null}
      </div>
      {state === "loading" ? <p className="vault-state vault-state--inline">Searching files...</p> : null}
      {state === "error" ? <p className="vault-state vault-state--error vault-state--inline">{error}</p> : null}
      {hasQuery && state === "idle" && results.length === 0 ? (
        <p className="vault-state vault-state--inline">No matching files.</p>
      ) : null}
      {results.length > 0 ? (
        <div className="vault-search__results">
          {results.map((node) => {
            const isSelected = selectedPath === node.path;
            return (
              <button
                type="button"
                className={`vault-search__result${isSelected ? " vault-search__result--selected" : ""}`}
                aria-current={isSelected ? "page" : undefined}
                key={node.path}
                onClick={() => onSelectFile(node)}
              >
                <FileIcon extension={node.extension ?? ""} />
                <span className="vault-search__result-name">{node.name}</span>
                <span className="vault-search__result-path">{node.path}</span>
              </button>
            );
          })}
          {truncated ? (
            <p className="vault-state vault-state--inline">Search truncated. Refine the query for more specific results.</p>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

function VaultExplorer({
  wards,
  state,
  error,
  wardsExpanded,
  selectedWard,
  wardChildren,
  childrenByPath,
  truncatedByPath,
  expanded,
  loadingPaths,
  selectedPath,
  onOpenWards,
  onSelectWard,
  onToggleDirectory,
  onSelectFile,
}: {
  wards: VaultWard[];
  state: LoadState;
  error: string | null;
  wardsExpanded: boolean;
  selectedWard: VaultWard | null;
  wardChildren: VaultNode[];
  childrenByPath: Record<string, VaultNode[]>;
  truncatedByPath: Record<string, boolean>;
  expanded: Set<string>;
  loadingPaths: Set<string>;
  selectedPath: string | null;
  onOpenWards: () => void;
  onSelectWard: (ward: VaultWard) => void;
  onToggleDirectory: (node: VaultNode) => void;
  onSelectFile: (node: VaultNode) => void;
}) {
  if (selectedWard) {
    return (
      <div className="vault-tree">
        <button
          type="button"
          aria-expanded
          aria-current={selectedPath == null ? "page" : undefined}
          className={`vault-tree-row${selectedPath == null ? " vault-tree-row--selected" : ""}`}
          style={{ paddingLeft: 8 }}
          onClick={() => onSelectWard(selectedWard)}
        >
          <FolderOpen size={16} />
          <span className="vault-tree-row__name">{selectedWard.name}</span>
        </button>
        <WardTree
          nodes={wardChildren}
          childrenByPath={childrenByPath}
          truncatedByPath={truncatedByPath}
          expanded={expanded}
          loadingPaths={loadingPaths}
          selectedPath={selectedPath}
          onToggle={onToggleDirectory}
          onSelectFile={onSelectFile}
          depth={1}
        />
        {truncatedByPath[""] ? (
          <p className="vault-state vault-state--inline">Directory truncated at 1,000 entries.</p>
        ) : null}
      </div>
    );
  }

  return (
    <div className="vault-tree">
      <button
        type="button"
        aria-expanded={wardsExpanded}
        className="vault-tree-row"
        style={{ paddingLeft: 8 }}
        onClick={onOpenWards}
      >
        {wardsExpanded ? <FolderOpen size={16} /> : <Folder size={16} />}
        <span className="vault-tree-row__name">Wards</span>
      </button>
      {wardsExpanded ? (
        <>
          {state === "loading" ? <p className="vault-state vault-state--inline">Loading wards...</p> : null}
          {state === "error" ? <p className="vault-state vault-state--error vault-state--inline">{error}</p> : null}
          {state === "idle" && wards.length === 0 ? (
            <p className="vault-state vault-state--inline">No wards found.</p>
          ) : null}
          {wards.map((ward) => (
            <div key={ward.id}>
              <button
                type="button"
                aria-expanded={false}
                className="vault-tree-row"
                style={{ paddingLeft: 26 }}
                onClick={() => onSelectWard(ward)}
              >
                <Folder size={16} />
                <span className="vault-tree-row__name">{ward.name}</span>
              </button>
            </div>
          ))}
        </>
      ) : null}
    </div>
  );
}

function WardTree({
  nodes,
  childrenByPath,
  truncatedByPath,
  expanded,
  loadingPaths,
  selectedPath,
  onToggle,
  onSelectFile,
  depth = 0,
}: {
  nodes: VaultNode[];
  childrenByPath: Record<string, VaultNode[]>;
  truncatedByPath: Record<string, boolean>;
  expanded: Set<string>;
  loadingPaths: Set<string>;
  selectedPath: string | null;
  onToggle: (node: VaultNode) => void;
  onSelectFile: (node: VaultNode) => void;
  depth?: number;
}) {
  return (
    <div className="vault-tree">
      {nodes.map((node) => {
        const isDirectory = node.kind === "directory";
        const isOpen = expanded.has(node.path);
        const isSelected = selectedPath === node.path;
        return (
          <div key={node.path || node.name}>
            <button
              type="button"
              aria-expanded={isDirectory ? isOpen : undefined}
              aria-current={isSelected ? "page" : undefined}
              className={`vault-tree-row${isSelected ? " vault-tree-row--selected" : ""}`}
              style={{ paddingLeft: `${8 + depth * 18}px` }}
              onClick={() => isDirectory ? onToggle(node) : onSelectFile(node)}
            >
              {isDirectory ? (
                isOpen ? <FolderOpen size={16} /> : <Folder size={16} />
              ) : (
                <FileIcon extension={node.extension ?? ""} />
              )}
              <span className="vault-tree-row__name">{node.name}</span>
            </button>
            {isDirectory && isOpen ? (
              <>
                {loadingPaths.has(node.path) ? (
                  <p className="vault-state vault-state--inline">Loading...</p>
                ) : null}
                <WardTree
                  nodes={childrenByPath[node.path] ?? []}
                  childrenByPath={childrenByPath}
                  truncatedByPath={truncatedByPath}
                  expanded={expanded}
                  loadingPaths={loadingPaths}
                  selectedPath={selectedPath}
                  onToggle={onToggle}
                  onSelectFile={onSelectFile}
                  depth={depth + 1}
                />
                {truncatedByPath[node.path] ? (
                  <p className="vault-state vault-state--inline">Directory truncated at 1,000 entries.</p>
                ) : null}
              </>
            ) : null}
          </div>
        );
      })}
      {depth === 0 && truncatedByPath[""] ? (
        <p className="vault-state vault-state--inline">Directory truncated at 1,000 entries.</p>
      ) : null}
    </div>
  );
}

function FileIcon({ extension }: { extension: string }) {
  if (extension === "md" || extension === "txt" || extension === "docx") return <FileText size={16} />;
  if (extension === "pptx" || extension === "ppt") return <Presentation size={16} />;
  if (["py", "js", "ts", "tsx", "html", "css", "json", "toml", "yaml", "yml"].includes(extension)) {
    return <Code2 size={16} />;
  }
  return <File size={16} />;
}

function FilePreviewPane({
  selected,
  ward,
  directoryCount,
  fileCount,
  onOpenWard,
}: {
  selected: SelectedFileState | null;
  ward: VaultWard;
  directoryCount: number;
  fileCount: number;
  onOpenWard: () => Promise<void>;
}) {
  if (!selected) {
    return (
      <section className="vault-preview-pane" aria-label="Vault file preview">
        <div className="vault-empty-preview">
          <div className="vault-empty-preview__glyph" aria-hidden="true">
            <FolderOpen size={34} />
          </div>
          <div>
            <p className="vault-empty-preview__eyebrow">Ward content</p>
            <p className="vault-state">{ward.name}</p>
            <p className="vault-empty-preview__meta">{directoryCount} dirs / {fileCount} files loaded</p>
          </div>
        </div>
      </section>
    );
  }

  const { node } = selected;
  return (
    <section className="vault-preview-pane" aria-label="Vault file preview">
      <header className="vault-preview-pane__head">
        <div>
          <h2>{node.name}</h2>
          <p>{node.path}</p>
        </div>
        {!node.previewable ? (
          <button className="btn btn--outline btn--sm" type="button" onClick={() => void onOpenWard()}>
            <FolderOpen size={14} />
            Open ward folder
          </button>
        ) : null}
      </header>
      <div className="vault-preview-pane__body">
        {selected.loading ? <p className="vault-state">Loading preview...</p> : null}
        {selected.error ? <p className="vault-state vault-state--error">{selected.error}</p> : null}
        {!selected.loading && !selected.error ? (
          <PreviewContent selected={selected} />
        ) : null}
      </div>
    </section>
  );
}

function PreviewContent({ selected }: { selected: SelectedFileState }) {
  const { node, content, officePreview } = selected;
  if (!node.previewable) {
    return <p className="vault-state">Preview not available for .{node.extension} files.</p>;
  }
  if (!content) return null;
  if (content.kind === "text") return <TextPreview file={content} />;
  if (content.kind === "office" && officePreview) {
    return <OfficePreviewView file={content} preview={officePreview} />;
  }
  return <p className="vault-state">Preview not available.</p>;
}

function TextPreview({ file }: { file: VaultTextFileResponse }) {
  if (file.extension === "md") {
    return <Markdown className="artifact-slideout__md vault-markdown">{file.content}</Markdown>;
  }
  if (file.extension === "html") {
    return (
      <iframe
        className="vault-html-preview"
        sandbox=""
        srcDoc={file.content}
        title={`HTML preview: ${file.name}`}
      />
    );
  }
  return (
    <pre className="vault-code-preview">
      <code>{file.content}</code>
    </pre>
  );
}

function OfficePreviewView({
  preview,
}: {
  file: VaultOfficeFileResponse;
  preview: OfficePreview;
}) {
  if (preview.kind === "docx") {
    return (
      <article className="vault-office-preview">
        {preview.blocks.map((block, index) => (
          block.type === "table" ? (
            <table className="vault-office-preview__table" key={index}>
              <tbody>
                {block.rows.map((row, rowIndex) => (
                  <tr key={rowIndex}>
                    {row.map((cell, cellIndex) => <td key={cellIndex}>{cell}</td>)}
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p key={index}>{block.text}</p>
          )
        ))}
      </article>
    );
  }
  if (preview.kind === "pptx") {
    return (
      <article className="vault-office-preview">
        {preview.slides.map((slide) => (
          <section className="vault-office-preview__slide" key={slide.number}>
            <h3>{slide.title}</h3>
            {slide.lines.slice(1).map((line, index) => <p key={index}>{line}</p>)}
          </section>
        ))}
      </article>
    );
  }

  return (
    <article className="vault-office-preview">
      {preview.sheets.map((sheet) => (
        <section className="vault-office-preview__slide" key={sheet.name}>
          <h3>{sheet.name}</h3>
          <table className="vault-office-preview__table">
            <tbody>
              {sheet.rows.map((row, rowIndex) => (
                <tr key={rowIndex}>
                  {row.map((cell, cellIndex) => <td key={cellIndex}>{cell}</td>)}
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      ))}
    </article>
  );
}
