import { describe, expect, it } from "vitest";
import JSZip from "jszip";
import {
  DEFAULT_OFFICE_PREVIEW_LIMITS,
  OfficePreviewLimitError,
  parseOfficePreview,
} from "./officePreview";

describe("parseOfficePreview", () => {
  it("extracts docx paragraphs and tables", async () => {
    const zip = new JSZip();
    zip.file("word/document.xml", `
      <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
        <w:body>
          <w:p>
            <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
            <w:r><w:t>Quarterly Plan</w:t></w:r>
          </w:p>
          <w:p>
            <w:pPr><w:numPr/></w:pPr>
            <w:r><w:t>Ship previewer</w:t></w:r>
          </w:p>
          <w:tbl>
            <w:tr>
              <w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc>
              <w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc>
            </w:tr>
          </w:tbl>
        </w:body>
      </w:document>
    `);

    const preview = await parseOfficePreview(await zip.generateAsync({ type: "arraybuffer" }), "docx");

    expect(preview.kind).toBe("docx");
    if (preview.kind !== "docx") return;
    expect(preview.blocks).toEqual([
      { type: "paragraph", text: "Quarterly Plan", style: "Heading1", list: false },
      { type: "paragraph", text: "Ship previewer", style: undefined, list: true },
      { type: "table", rows: [["Name", "Status"]] },
    ]);
  });

  it("extracts xlsx sheets with shared strings and inline strings", async () => {
    const zip = new JSZip();
    zip.file("xl/workbook.xml", `
      <workbook xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
        <sheets><sheet name="Tasks" r:id="rId1"/></sheets>
      </workbook>
    `);
    zip.file("xl/_rels/workbook.xml.rels", `
      <Relationships>
        <Relationship Id="rId1" Target="worksheets/sheet1.xml"/>
      </Relationships>
    `);
    zip.file("xl/sharedStrings.xml", `
      <sst>
        <si><t>Name</t></si>
        <si><t>Build</t></si>
      </sst>
    `);
    zip.file("xl/worksheets/sheet1.xml", `
      <worksheet>
        <sheetData>
          <row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="inlineStr"><is><t>Status</t></is></c></row>
          <row r="2"><c r="A2" t="s"><v>1</v></c><c r="B2"><v>1</v></c></row>
        </sheetData>
      </worksheet>
    `);

    const preview = await parseOfficePreview(await zip.generateAsync({ type: "arraybuffer" }), "xlsx");

    expect(preview.kind).toBe("xlsx");
    if (preview.kind !== "xlsx") return;
    expect(preview.sheets).toEqual([
      { name: "Tasks", rows: [["Name", "Status"], ["Build", "1"]] },
    ]);
  });

  it("extracts pptx slide text", async () => {
    const zip = new JSZip();
    zip.file("ppt/slides/slide2.xml", `
      <p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:cSld><p:spTree><p:sp><p:txBody>
          <a:p><a:r><a:t>Second Slide</a:t></a:r></a:p>
          <a:p><a:r><a:t>More detail</a:t></a:r></a:p>
        </p:txBody></p:sp></p:spTree></p:cSld>
      </p:sld>
    `);
    zip.file("ppt/slides/slide1.xml", `
      <p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:cSld><p:spTree><p:sp><p:txBody>
          <a:p><a:r><a:t>First Slide</a:t></a:r></a:p>
        </p:txBody></p:sp></p:spTree></p:cSld>
      </p:sld>
    `);

    const preview = await parseOfficePreview(await zip.generateAsync({ type: "arraybuffer" }), "pptx");

    expect(preview.kind).toBe("pptx");
    if (preview.kind !== "pptx") return;
    expect(preview.slides).toEqual([
      { number: 1, title: "First Slide", lines: ["First Slide"] },
      { number: 2, title: "Second Slide", lines: ["Second Slide", "More detail"] },
    ]);
  });

  it("rejects Office files with too many zip entries", async () => {
    const zip = new JSZip();
    zip.file("word/document.xml", "<document />");
    for (let i = 0; i < 257; i += 1) {
      zip.file(`extra-${i}.xml`, "<x />");
    }

    await expect(parseOfficePreview(
      await zip.generateAsync({ type: "arraybuffer" }),
      "docx",
    )).rejects.toBeInstanceOf(OfficePreviewLimitError);
  });

  it("rejects Office files that exceed XML byte limits", async () => {
    const zip = new JSZip();
    zip.file("word/document.xml", `
      <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
        <w:body><w:p><w:r><w:t>Too much XML</w:t></w:r></w:p></w:body>
      </w:document>
    `);

    await expect(parseOfficePreview(
      await zip.generateAsync({ type: "arraybuffer" }),
      "docx",
      { ...DEFAULT_OFFICE_PREVIEW_LIMITS, maxXmlBytes: 8 },
    )).rejects.toBeInstanceOf(OfficePreviewLimitError);
  });

  it("rejects pptx files that exceed slide limits", async () => {
    const zip = new JSZip();
    zip.file("ppt/slides/slide1.xml", "<p:sld />");
    zip.file("ppt/slides/slide2.xml", "<p:sld />");

    await expect(parseOfficePreview(
      await zip.generateAsync({ type: "arraybuffer" }),
      "pptx",
      { ...DEFAULT_OFFICE_PREVIEW_LIMITS, maxPptxSlides: 1 },
    )).rejects.toBeInstanceOf(OfficePreviewLimitError);
  });

  it("rejects Office files that exceed extracted text limits", async () => {
    const zip = new JSZip();
    zip.file("word/document.xml", `
      <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
        <w:body><w:p><w:r><w:t>abcdefghij</w:t></w:r></w:p></w:body>
      </w:document>
    `);

    await expect(parseOfficePreview(
      await zip.generateAsync({ type: "arraybuffer" }),
      "docx",
      { ...DEFAULT_OFFICE_PREVIEW_LIMITS, maxExtractedTextChars: 5 },
    )).rejects.toBeInstanceOf(OfficePreviewLimitError);
  });
});
