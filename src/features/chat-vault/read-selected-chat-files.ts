// @story 04.6
// @implements AC-02, AC-04 (Chat File Intake Helper for Text Snippets & Binary CAS Files)
const MAX_TEXT_SNIPPET_CHARS = 8000;
const TEXT_READABLE_EXT_RE = /\.(md|txt|html|htm|css|json|svg|csv|tsv|ts|tsx|js|jsx|yaml|yml)$/i;

export const ALLOWED_CHAT_ATTACHMENT_ACCEPT =
  ".csv,.tsv,.xlsx,.fig,.pdf,.png,.svg,.txt,.md,.json";

const DDE_TRIGGER_CHARS = new Set(["=", "+", "-", "@", "\t", "\r"]);

function neutralizeCell(cell: string): string {
  if (cell.length > 0 && DDE_TRIGGER_CHARS.has(cell[0])) {
    return `'${cell}`;
  }
  if (cell.length > 1 && cell[0] === "\"" && DDE_TRIGGER_CHARS.has(cell[1])) {
    return `"'${cell.slice(1)}`;
  }
  return cell;
}

export function neutralizeCsvTsvText(raw: string, filename: string): string {
  if (!/\.(csv|tsv)$/i.test(filename)) return raw;
  const delim = /\.tsv$/i.test(filename) ? "\t" : ",";
  return raw
    .split("\n")
    .map((line) => line.split(delim).map(neutralizeCell).join(delim))
    .join("\n");
}

export async function readSelectedChatFiles(
  files: File[]
): Promise<{ names: string[]; snippets: Record<string, string>; binaryFiles: File[] }> {
  const names = files.map((file) => file.name);
  const snippets: Record<string, string> = {};
  const binaryFiles: File[] = [];

  await Promise.all(
    files.map(async (file) => {
      binaryFiles.push(file);
      if (!TEXT_READABLE_EXT_RE.test(file.name) || typeof file.text !== "function") return;
      try {
        const rawText = await file.text();
        if (rawText.trim()) {
          const safeText = neutralizeCsvTsvText(rawText, file.name);
          snippets[file.name] = safeText.slice(0, MAX_TEXT_SNIPPET_CHARS);
        }
      } catch {
        // Ignore unreadable text slice while preserving filename and binary handle
      }
    })
  );
  return { names, snippets, binaryFiles };
}
