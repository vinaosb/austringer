---
name: markitdown
description: "Microsoft MarkItDown file-to-Markdown converter with MCP server and OCR plugin. Converts PDF, DOCX, PPTX, XLS, images, audio, HTML, EPUB, and more to Markdown. MCP tool `convert_to_markdown(uri)` handles file://, http://, https://, data:// URIs. OCR plugin extracts text from embedded images via LLM Vision. USE FOR: converting documents to markdown, extracting text from PDFs, reading Word/Excel/PPT files, OCR on scanned documents, document analysis pipelines. DO NOT USE FOR: editing documents, creating files from scratch, image generation."
license: MIT
metadata:
  author: microsoft
  version: "0.1.5"
  packages: ["markitdown==0.1.5", "markitdown-mcp==0.0.1a4", "markitdown-ocr==0.1.0"]
  mcp_server: markitdown
---

# MarkItDown — File-to-Markdown Converter

Microsoft's lightweight Python utility for converting files to Markdown. Installed with MCP server and OCR plugin.

---

## MCP Tool: `convert_to_markdown`

The MCP server exposes one tool. Use it for any file conversion:

**Tool:** `convert_to_markdown(uri)`

| Parameter | Type | Description |
|-----------|------|-------------|
| `uri` | string | `file://`, `http://`, `https://`, or `data:` URI to convert |

**Examples:**
- Local file: `file:///C:/Users/user/document.pdf`
- URL: `https://example.com/report.docx`
- Data URI: `data:text/plain;base64,...`

**Output:** Markdown text content of the converted document.

---

## Supported Formats

| Format | Extensions | Notes |
|--------|-----------|-------|
| PDF | `.pdf` | Text extraction, tables, links |
| Word | `.docx` | Headings, lists, tables |
| PowerPoint | `.pptx` | Slide content, speaker notes |
| Excel | `.xlsx`, `.xls` | Sheet data as markdown tables |
| HTML | `.html`, `.htm` | Full structure preserved |
| Images | `.jpg`, `.png`, `.gif`, etc. | EXIF metadata + image description (with LLM) |
| Audio | `.wav`, `.mp3` | EXIF metadata + transcription (with speech rec) |
| CSV/JSON/XML | various | Direct markdown conversion |
| EPUB | `.epub` | Book content |
| ZIP | `.zip` | Iterates over contents |
| YouTube | URLs | Transcript extraction |

---

## CLI Usage (Direct)

The `markitdown` CLI is available at:
```
C:\Users\vsbb1\AppData\Roaming\Python\Python314\Scripts\markitdown.exe
```

```bash
# Convert a file to markdown
markitdown path-to-file.pdf > output.md

# With output file
markitdown path-to-file.pdf -o output.md

# Pipe content
cat file.pdf | markitdown

# Enable plugins (for OCR)
markitdown document.pdf --use-plugins

# List installed plugins
markitdown --list-plugins
```

---

## Python API (Direct)

```python
from markitdown import MarkItDown

# Basic conversion
md = MarkItDown()
result = md.convert("document.pdf")
print(result.text_content)

# With plugins enabled (for OCR)
md = MarkItDown(enable_plugins=True)
result = md.convert("scanned_document.pdf")

# With LLM for image descriptions
from openai import OpenAI
md = MarkItDown(
    llm_client=OpenAI(base_url="http://127.0.0.1:8188/v1"),
    llm_model="local-model",
)
result = md.convert("presentation.pptx")
```

---

## OCR Plugin (markitdown-ocr)

Extracts text from embedded images in documents using LLM Vision.

**How it works:**
1. Detects images embedded in PDF/DOCX/PPTX/XLSX
2. Sends each image to the LLM with an extraction prompt
3. Inserts extracted text inline, wrapped as: `*[Image OCR]...[End OCR]*`
4. Scanned PDFs (no text layer) get full-page OCR at 300 DPI

**Requirements:** `llm_client` + `llm_model` must be provided. Without them, OCR is silently skipped.

**With localhost model:**
```python
from markitdown import MarkItDown
from openai import OpenAI

md = MarkItDown(
    enable_plugins=True,
    llm_client=OpenAI(base_url="http://127.0.0.1:8188/v1"),
    llm_model="gemma",  # or whatever local model is running
)
result = md.convert("scanned_invoice.pdf")
```

**CLI with OCR:**
```bash
markitdown document.pdf --use-plugins --llm-client openai --llm-model gemma
```

---

## Agent Usage Patterns

### Pattern 1: MCP Tool (Preferred for agents)
Use the `markitdown` MCP server's `convert_to_markdown` tool. This is the simplest approach — just pass a file URI.

```
# Convert a local file
convert_to_markdown(uri="file:///C:/path/to/document.pdf")

# Convert from URL
convert_to_markdown(uri="https://example.com/report.docx")
```

### Pattern 2: Python Script (For complex pipelines)
When MCP isn't available or you need fine-grained control (OCR, custom prompts, etc.), write a Python script:

```python
from markitdown import MarkItDown
from openai import OpenAI

md = MarkItDown(
    enable_plugins=True,
    llm_client=OpenAI(base_url="http://127.0.0.1:8188/v1"),
    llm_model="gemma",
)
result = md.convert("path/to/file.pdf")
print(result.text_content)
```

---

## Installation Info

| Package | Version | Path |
|---------|---------|------|
| `markitdown` | 0.1.5 | `C:\Users\vsbb1\AppData\Roaming\Python\Python314\site-packages\markitdown` |
| `markitdown-mcp` | 0.0.1a4 | `C:\Users\vsbb1\AppData\Roaming\Python\Python314\site-packages\markitdown_mcp` |
| `markitdown-ocr` | 0.1.0 | `C:\Users\vsbb1\AppData\Roaming\Python\Python314\site-packages\markitdown_ocr` |
| `mcp` | 1.8.1 | MCP SDK |
| `openai` | 2.31.0 | OpenAI client (already installed) |

**MCP Server Config** (in `~/.config/opencode/opencode.json`):
```json
"markitdown": {
  "type": "local",
  "command": ["C:\\Users\\vsbb1\\AppData\\Roaming\\Python\\Python314\\Scripts\\markitdown-mcp.exe"],
  "enabled": true,
  "timeout": 120000
}
```

**Note:** The Scripts directory is NOT on PATH. Use full paths for CLI tools.

---

## Troubleshooting

| Issue | Fix |
|-------|-----|
| OCR text missing | Ensure `llm_client` and `llm_model` are provided |
| Plugin not loading | Run `markitdown --list-plugins` — should show `ocr` |
| ffmpeg warning | Install ffmpeg for audio transcription (optional) |
| API errors | Check model is running at `http://127.0.0.1:8188/v1` |
| File not found | Use absolute paths or `file:///` URIs |
