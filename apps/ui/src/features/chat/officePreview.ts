import JSZip from "jszip";

export type DocumentBlock =
  | { type: "paragraph"; text: string; style?: string; list?: boolean }
  | { type: "table"; rows: string[][] };

export interface DocumentPreview {
  kind: "docx";
  blocks: DocumentBlock[];
}

export interface WorkbookSheetPreview {
  name: string;
  rows: string[][];
}

export interface WorkbookPreview {
  kind: "xlsx";
  sheets: WorkbookSheetPreview[];
}

export interface PresentationSlidePreview {
  number: number;
  title: string;
  lines: string[];
}

export interface PresentationPreview {
  kind: "pptx";
  slides: PresentationSlidePreview[];
}

export type OfficePreview = DocumentPreview | WorkbookPreview | PresentationPreview;

export interface OfficePreviewLimits {
  maxZipEntries: number;
  maxXmlBytes: number;
  maxPptxSlides: number;
  maxExtractedTextChars: number;
}

export const DEFAULT_OFFICE_PREVIEW_LIMITS: OfficePreviewLimits = {
  maxZipEntries: 256,
  maxXmlBytes: 25 * 1024 * 1024,
  maxPptxSlides: 300,
  maxExtractedTextChars: 200_000,
};

export class OfficePreviewLimitError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "OfficePreviewLimitError";
  }
}

interface ParseContext {
  limits: OfficePreviewLimits;
  xmlBytes: number;
  extractedTextChars: number;
}

export async function parseOfficePreview(
  data: ArrayBuffer,
  fileType: "docx" | "xlsx" | "pptx",
  limits: OfficePreviewLimits = DEFAULT_OFFICE_PREVIEW_LIMITS,
): Promise<OfficePreview> {
  const zip = await JSZip.loadAsync(data);
  const entries = Object.keys(zip.files);
  if (entries.length > limits.maxZipEntries) {
    throw new OfficePreviewLimitError(`Office preview has too many zip entries (${entries.length})`);
  }
  const context: ParseContext = { limits, xmlBytes: 0, extractedTextChars: 0 };
  if (fileType === "docx") return parseDocx(zip, context);
  if (fileType === "xlsx") return parseXlsx(zip, context);
  return parsePptx(zip, context);
}

async function parseDocx(zip: JSZip, context: ParseContext): Promise<DocumentPreview> {
  const xml = await readZipText(zip, "word/document.xml", context);
  const doc = parseXml(xml);
  const body = firstByLocalName(doc, "body");
  const blocks: DocumentBlock[] = [];

  for (const child of Array.from(body?.children ?? [])) {
    if (child.localName === "p") {
      const text = paragraphText(child, context);
      if (!text) continue;
      blocks.push({
        type: "paragraph",
        text,
        style: paragraphStyle(child),
        list: Boolean(firstByLocalName(child, "numPr")),
      });
    }

    if (child.localName === "tbl") {
      const rows = Array.from(child.getElementsByTagNameNS("*", "tr"))
        .map((row) =>
          Array.from(row.getElementsByTagNameNS("*", "tc"))
            .map((cell) => paragraphText(cell, context).trim())
        )
        .filter((row) => row.some(Boolean));
      if (rows.length > 0) blocks.push({ type: "table", rows });
    }
  }

  return { kind: "docx", blocks };
}

async function parseXlsx(zip: JSZip, context: ParseContext): Promise<WorkbookPreview> {
  const workbook = parseXml(await readZipText(zip, "xl/workbook.xml", context));
  const rels = parseXml(await readZipText(zip, "xl/_rels/workbook.xml.rels", context));
  const sharedStrings = await readSharedStrings(zip, context);
  const relationshipTargets = new Map<string, string>();

  for (const rel of Array.from(rels.getElementsByTagNameNS("*", "Relationship"))) {
    const id = rel.getAttribute("Id");
    const target = rel.getAttribute("Target");
    if (id && target) relationshipTargets.set(id, normalizeWorkbookTarget(target));
  }

  const sheets: WorkbookSheetPreview[] = [];
  for (const sheet of Array.from(workbook.getElementsByTagNameNS("*", "sheet"))) {
    const name = sheet.getAttribute("name") || "Sheet";
    const relationshipId = sheet.getAttribute("r:id");
    const worksheetPath = relationshipId ? relationshipTargets.get(relationshipId) : null;
    if (!worksheetPath) continue;

    const worksheetXml = await readZipText(zip, worksheetPath, context);
    const worksheet = parseXml(worksheetXml);
    const rows = Array.from(worksheet.getElementsByTagNameNS("*", "row"))
      .slice(0, 100)
      .map((row) => worksheetRow(row, sharedStrings, context))
      .filter((row) => row.some(Boolean));
    sheets.push({ name, rows });
  }

  return { kind: "xlsx", sheets };
}

async function parsePptx(zip: JSZip, context: ParseContext): Promise<PresentationPreview> {
  const slidePaths = Object.keys(zip.files)
    .filter((path) => /^ppt\/slides\/slide\d+\.xml$/.test(path))
    .sort((a, b) => slideNumber(a) - slideNumber(b));
  if (slidePaths.length > context.limits.maxPptxSlides) {
    throw new OfficePreviewLimitError(`Office preview has too many slides (${slidePaths.length})`);
  }

  const slides: PresentationSlidePreview[] = [];
  for (const path of slidePaths) {
    const slide = parseXml(await readZipText(zip, path, context));
    const lines = Array.from(slide.getElementsByTagNameNS("*", "p"))
      .map((paragraph) => paragraphText(paragraph, context))
      .filter(Boolean);
    const number = slideNumber(path);
    slides.push({
      number,
      title: lines[0] || `Slide ${number}`,
      lines,
    });
  }

  return { kind: "pptx", slides };
}

async function readSharedStrings(zip: JSZip, context: ParseContext): Promise<string[]> {
  const file = zip.file("xl/sharedStrings.xml");
  if (!file) return [];
  const doc = parseXml(await readZipText(zip, "xl/sharedStrings.xml", context));
  return Array.from(doc.getElementsByTagNameNS("*", "si")).map((item) => paragraphText(item, context));
}

function worksheetRow(row: Element, sharedStrings: string[], context: ParseContext): string[] {
  const cells: string[] = [];
  for (const cell of Array.from(row.getElementsByTagNameNS("*", "c"))) {
    const ref = cell.getAttribute("r");
    const col = ref ? columnIndex(ref) : cells.length;
    cells[col] = cellValue(cell, sharedStrings, context);
  }
  return Array.from({ length: cells.length }, (_, index) => cells[index] ?? "");
}

function cellValue(cell: Element, sharedStrings: string[], context: ParseContext): string {
  const type = cell.getAttribute("t");
  if (type === "inlineStr") {
    const inline = firstByLocalName(cell, "is");
    return inline ? paragraphText(inline, context) : "";
  }

  const raw = firstByLocalName(cell, "v")?.textContent ?? "";
  if (type === "s") return sharedStrings[Number(raw)] ?? "";
  return raw;
}

function paragraphText(element: Element, context: ParseContext): string {
  const parts: string[] = [];
  for (const node of Array.from(element.getElementsByTagNameNS("*", "t"))) {
    const text = node.textContent ?? "";
    if (text) parts.push(text);
  }
  const value = parts.join("").replace(/\s+/g, " ").trim();
  context.extractedTextChars += value.length;
  if (context.extractedTextChars > context.limits.maxExtractedTextChars) {
    throw new OfficePreviewLimitError("Office preview extracted text is too large");
  }
  return value;
}

function paragraphStyle(paragraph: Element): string | undefined {
  const style = firstByLocalName(paragraph, "pStyle");
  return style?.getAttribute("w:val") ?? style?.getAttribute("val") ?? undefined;
}

function parseXml(xml: string): Document {
  return new DOMParser().parseFromString(xml, "application/xml");
}

async function readZipText(zip: JSZip, path: string, context: ParseContext): Promise<string> {
  const file = zip.file(path);
  if (!file) throw new Error(`Missing ${path}`);
  const text = await file.async("text");
  context.xmlBytes += new TextEncoder().encode(text).length;
  if (context.xmlBytes > context.limits.maxXmlBytes) {
    throw new OfficePreviewLimitError("Office preview XML is too large");
  }
  return text;
}

function firstByLocalName(element: Document | Element, localName: string): Element | undefined {
  return Array.from(element.getElementsByTagNameNS("*", localName))[0];
}

function normalizeWorkbookTarget(target: string): string {
  if (target.startsWith("/")) return target.slice(1);
  if (target.startsWith("xl/")) return target;
  return `xl/${target}`;
}

function columnIndex(cellRef: string): number {
  const letters = cellRef.match(/[A-Z]+/i)?.[0]?.toUpperCase() ?? "A";
  return [...letters].reduce((acc, letter) => acc * 26 + letter.charCodeAt(0) - 64, 0) - 1;
}

function slideNumber(path: string): number {
  return Number(path.match(/slide(\d+)\.xml$/)?.[1] ?? 0);
}
