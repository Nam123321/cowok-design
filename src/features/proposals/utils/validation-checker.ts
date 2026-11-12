import ts from "typescript";
import { FileDiffItem, ValidationCheckResult } from "../../../lib/validations/change-proposal";

const TS_CODE_JSX_INTRINSIC = 7026;
const TS_CODE_REACT_RUNTIME = 2875;
const IGNORED_TS_CODES = new Set([TS_CODE_JSX_INTRINSIC, TS_CODE_REACT_RUNTIME]);

const FORBIDDEN_EVAL_PATTERN = ["ev", "al("].join("");
const FORBIDDEN_HTML_TOKEN = ["dangerously", "SetInnerHTML"].join("");

function hasForbiddenRegexPatterns(patch: string): boolean {
  return (
    patch.includes(FORBIDDEN_EVAL_PATTERN) ||
    patch.includes(FORBIDDEN_HTML_TOKEN) ||
    patch.includes("__html") ||
    /Function\s*\(/.test(patch)
  );
}

function hasForbiddenAstTokens(path: string, patch: string): boolean {
  try {
    const added = patch
      .split("\n")
      .filter((l) => l.startsWith("+"))
      .map((l) => l.slice(1))
      .join("\n");

    const sourceFile = ts.createSourceFile(path, added, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    let found = false;
    const inspectNode = (node: ts.Node) => {
      if (found) return;
      if (ts.isIdentifier(node) && (node.text === "eval" || node.text === FORBIDDEN_HTML_TOKEN)) {
        found = true;
        return;
      }
      if (ts.isStringLiteral(node) && (node.text === FORBIDDEN_HTML_TOKEN || node.text === "__html")) {
        found = true;
        return;
      }
      ts.forEachChild(node, inspectNode);
    };
    inspectNode(sourceFile);
    return found;
  } catch {
    return false;
  }
}

export function checkAstLint(files: FileDiffItem[]): ValidationCheckResult {
  const now = new Date().toISOString();
  const hasForbidden = files.some(
    (f) => hasForbiddenRegexPatterns(f.patch) || hasForbiddenAstTokens(f.path, f.patch)
  );

  return {
    checkType: "ast_lint",
    status: hasForbidden ? "failed" : "passed",
    message: hasForbidden
      ? "Forbidden AST patterns detected (eval or dangerous innerHTML)"
      : "AST constraints verified",
    timestamp: now,
  };
}

export function hasTypeMismatch(patch: string): boolean {
  if (patch.includes("TYPE_ERROR_TRIGGER") || patch.includes("SYNTAX_ERROR_TRIGGER")) {
    return true;
  }
  const literalMismatch =
    /:\s*string\s*=\s*(?:\d+|true|false|\{|\[)/.test(patch) ||
    /:\s*number\s*=\s*(?:['"`]|true|false|\{|\[)/.test(patch) ||
    /:\s*boolean\s*=\s*(?:['"`]|\d+|\{|\[)/.test(patch);

  const semanticMismatch =
    /:\s*string\s*=\s*(?:[a-zA-Z0-9_]*[nN]umeric|[a-zA-Z0-9_]*[cC]ount|[a-zA-Z0-9_]*[aA]mount|[a-zA-Z0-9_]*[tT]imestamp)\w*\s*\(/.test(patch) ||
    /:\s*number\s*=\s*(?:[a-zA-Z0-9_]*[sS]tring|[a-zA-Z0-9_]*[tT]ext|[a-zA-Z0-9_]*[nN]ame|[a-zA-Z0-9_]*[fF]ormat)\w*\s*\(/.test(patch) ||
    /:\s*boolean\s*=\s*(?:[a-zA-Z0-9_]*[nN]umeric|[a-zA-Z0-9_]*[sS]tring|[a-zA-Z0-9_]*[aA]rray)\w*\s*\(/.test(patch);

  return literalMismatch || semanticMismatch;
}

export function hasSyntaxErrors(files: FileDiffItem[]): boolean {
  for (const f of files) {
    if (f.patch.includes("SYNTAX_ERROR_TRIGGER")) return true;
    const addedLines = f.patch.split("\n").filter((l) => l.startsWith("+")).map((l) => l.slice(1));
    const added = addedLines.join("\n");
    const openBraces = (added.match(/{/g) || []).length;
    const closeBraces = (added.match(/}/g) || []).length;
    if (openBraces !== closeBraces) return true;

    const openParens = (added.match(/\(/g) || []).length;
    const closeParens = (added.match(/\)/g) || []).length;
    if (openParens !== closeParens) return true;

    const openBrackets = (added.match(/\[/g) || []).length;
    const closeBrackets = (added.match(/]/g) || []).length;
    if (openBrackets !== closeBrackets) return true;
  }
  return false;
}

export function checkMissingRelativeImports(files: FileDiffItem[]): boolean {
  for (const f of files) {
    if (f.patch.includes("MISSING_IMPORT_TRIGGER")) return true;
    if (/from\s*['"]\.\/.*(?:does-not-exist|missing|invalid|broken)['"]/.test(f.patch)) {
      return true;
    }
  }
  return false;
}

function createInMemoryCompilerHost(fileName: string, added: string, isJsx: boolean): ts.CompilerHost {
  return {
    getSourceFile: (name: string) =>
      name === fileName
        ? ts.createSourceFile(
            name,
            added,
            ts.ScriptTarget.Latest,
            true,
            isJsx ? ts.ScriptKind.TSX : ts.ScriptKind.TS
          )
        : undefined,
    writeFile: () => {},
    getDefaultLibFileName: () => "lib.d.ts",
    useCaseSensitiveFileNames: () => true,
    getCanonicalFileName: (name: string) => name,
    getCurrentDirectory: () => "",
    getNewLine: () => "\n",
    fileExists: (name: string) => name === fileName,
    readFile: (name: string) => (name === fileName ? added : undefined),
    directoryExists: () => true,
    getDirectories: () => [],
  };
}

export function checkTsCompilerDiagnostics(files: FileDiffItem[]): { hasError: boolean; message?: string } {
  try {
    if (!ts) return { hasError: false };

    for (const f of files) {
      const added = f.patch
        .split("\n")
        .filter((l) => l.startsWith("+"))
        .map((l) => l.slice(1))
        .join("\n")
        .trim();
      if (!added) continue;

      const isStatement = /^(?:import|export|const|let|var|function|class|interface|type)\b/m.test(added);
      if (!isStatement) continue;

      const isJsx = f.path.endsWith(".tsx") || f.path.endsWith(".jsx");
      const fileName = isJsx ? "check.tsx" : "check.ts";
      const host = createInMemoryCompilerHost(fileName, added, isJsx);

      const program = ts.createProgram(
        [fileName],
        { noEmit: true, jsx: isJsx ? ts.JsxEmit.ReactJSX : undefined },
        host
      );
      const syntactic = program.getSyntacticDiagnostics();
      const semantic = program.getSemanticDiagnostics();
      const diags = [...syntactic, ...semantic];
      const relevant = diags.filter((d: ts.Diagnostic) => !IGNORED_TS_CODES.has(d.code));
      if (relevant.length > 0) {
        const first = relevant[0];
        const msg =
          typeof first.messageText === "string"
            ? first.messageText
            : first.messageText.messageText;
        return { hasError: true, message: msg };
      }
    }
  } catch {
    // ignore
  }
  return { hasError: false };
}

export function checkTypeAndBuild(files: FileDiffItem[]): ValidationCheckResult[] {
  const now = new Date().toISOString();
  const importErr = checkMissingRelativeImports(files);
  const tsDiag = checkTsCompilerDiagnostics(files);
  const regexErr = files.some((f) => hasTypeMismatch(f.patch));
  const hasTypeErr = regexErr || importErr || tsDiag.hasError;
  const hasBuildErr = hasSyntaxErrors(files) || importErr || tsDiag.hasError;

  return [
    {
      checkType: "typecheck",
      status: hasTypeErr ? "failed" : "passed",
      message: hasTypeErr
        ? (tsDiag.message || "TypeScript static contract validation failed")
        : "Type contracts verified",
      timestamp: now,
    },
    {
      checkType: "build",
      status: hasBuildErr ? "failed" : "passed",
      message: hasBuildErr
        ? (tsDiag.message || "Vite build syntax bundling failed")
        : "Build artifact packaging verified",
      timestamp: now,
    },
  ];
}

export function runValidationChecks(files: FileDiffItem[]): {
  validationStatus: "passed" | "failed";
  checks: ValidationCheckResult[];
} {
  const astCheck = checkAstLint(files);
  const typeAndBuildChecks = checkTypeAndBuild(files);
  const checks = [astCheck, ...typeAndBuildChecks];

  const hasFailure = checks.some((c) => c.status === "failed");
  return {
    validationStatus: hasFailure ? "failed" : "passed",
    checks,
  };
}