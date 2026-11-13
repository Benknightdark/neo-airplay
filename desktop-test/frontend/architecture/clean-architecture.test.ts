/**
 * @file Clean Architecture 依賴邊界測試，驗證前後端各層僅遵守由外向內的依賴方向。
 */
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/** 目前測試檔案所在目錄。 */
const currentDir = dirname(fileURLToPath(import.meta.url));
/** 專案根目錄路徑。 */
const repoRoot = resolve(currentDir, "../../..");
/** 前端原始碼目錄路徑。 */
const frontendSrc = join(repoRoot, "desktop/src");
/** 後端 Rust 原始碼目錄路徑。 */
const backendSrc = join(repoRoot, "desktop/src-tauri/src");

/**
 * 遞迴蒐集指定目錄下符合副檔名清單的原始碼檔案路徑。
 *
 * @param directory 要掃描的目標目錄路徑。
 * @param extensions 允許納入結果的副檔名清單（含點號，例如 `.ts`）。
 * @returns 所有符合副檔名條件的完整檔案路徑陣列。
 */
function collectFiles(directory: string, extensions: readonly string[]): string[] {
  const entries = readdirSync(directory, { withFileTypes: true });
  const files: string[] = [];
  for (const entry of entries) {
    const fullPath = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectFiles(fullPath, extensions));
    } else if (extensions.some((extension) => entry.name.endsWith(extension))) {
      files.push(fullPath);
    }
  }
  return files;
}

/**
 * 移除原始碼中的單行與區塊註解，同時保留字串與樣板字面值內容，避免註解文字干擾靜態掃描。
 *
 * @param source 原始碼文字內容。
 * @returns 剝除註解後的原始碼文字。
 */
function stripComments(source: string): string {
  return source.replace(
    /\/\*[\s\S]*?\*\/|\/\/.*$|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`/gm,
    (token) => (token.startsWith("/") ? "" : token),
  );
}

/**
 * 從 TypeScript 或 Vue 原始碼擷取所有靜態 `import` 與轉匯 `export ... from` 模組路徑。
 *
 * @param source 待解析的檔案文字內容。
 * @returns 依出現順序蒐集的模組指定符（module specifier）陣列。
 */
function extractTsImports(source: string): string[] {
  const imports: string[] = [];
  const pattern = /(?:import|export)\s+(?:type\s+)?(?:[^"']*from\s+)?["']([^"']+)["']/g;
  for (const match of stripComments(source).matchAll(pattern)) {
    if (match[1]) imports.push(match[1]);
  }
  return imports;
}

/** 驗證前後端 Clean Architecture 各層依賴邊界不得逆向或跨層滲漏。 */
describe("Clean Architecture 依賴邊界守門員", () => {
  /** 驗證前端領域層只包含純 TypeScript 邏輯，不依賴外層模組、Vue 或 Tauri。 */
  it("前端 domain 層不依賴外層模組、Vue 或 Tauri", () => {
    const files = collectFiles(join(frontendSrc, "domain"), [".ts"]);
    expect(files.length).toBeGreaterThan(0);

    for (const file of files) {
      const imports = extractTsImports(readFileSync(file, "utf8"));
      for (const specifier of imports) {
        expect(specifier, `${file} 不可引用 ${specifier}`).not.toMatch(
          /(?:application|infrastructure|presentation|composables|components|api|vue|@tauri-apps)/,
        );
      }
    }
  });

  /** 驗證前端應用層僅可向上依賴 domain 層，禁止耦合基礎設施或 UI 層。 */
  it("前端 application 層只依賴 domain 層，不依賴外層或框架", () => {
    const files = collectFiles(join(frontendSrc, "application"), [".ts"]);
    expect(files.length).toBeGreaterThan(0);

    for (const file of files) {
      const imports = extractTsImports(readFileSync(file, "utf8"));
      for (const specifier of imports) {
        expect(specifier, `${file} 不可引用 ${specifier}`).not.toMatch(
          /(?:infrastructure|presentation|composables|components|api|vue|@tauri-apps)/,
        );
      }
    }
  });

  /** 驗證前端 Vue 頁面與元件未繞過表現層或應用層直接依賴 api 或 Tauri SDK。 */
  it("前端 Vue 元件不直接依賴 api、infrastructure 或 Tauri SDK", () => {
    const files = collectFiles(join(frontendSrc, "components"), [".vue"]);
    expect(files.length).toBeGreaterThan(0);

    for (const file of files) {
      const imports = extractTsImports(readFileSync(file, "utf8"));
      for (const specifier of imports) {
        expect(specifier, `${file} 不可直接引用 ${specifier}`).not.toMatch(
          /(?:\.\.\/api|infrastructure|@tauri-apps)/,
        );
      }
    }
  });

  /** 驗證後端 Rust domain 層完全與外層模組、Tauri 及標準 I/O 隔離。 */
  it("後端 Rust domain 層不依賴外層模組、Tauri、檔案系統或程序模組", () => {
    const files = collectFiles(join(backendSrc, "domain"), [".rs"]);
    expect(files.length).toBeGreaterThan(0);

    for (const file of files) {
      const source = readFileSync(file, "utf8");
      expect(source, `${file} 不可依賴外層模組`).not.toMatch(
        /crate::(?:application|infrastructure|presentation)/,
      );
      expect(source, `${file} 不可依賴 tauri 或 I/O`).not.toMatch(
        /(?:use\s+tauri|std::fs|std::process::Command)/,
      );
    }
  });

  /** 驗證後端 Rust application 層未反向依賴基礎設施、表現層或系統 I/O。 */
  it("後端 Rust application 層不依賴 infrastructure、presentation、Tauri 或 I/O", () => {
    const files = collectFiles(join(backendSrc, "application"), [".rs"]);
    expect(files.length).toBeGreaterThan(0);

    for (const file of files) {
      const source = readFileSync(file, "utf8");
      expect(source, `${file} 不可依賴外層模組`).not.toMatch(
        /crate::(?:infrastructure|presentation)/,
      );
      expect(source, `${file} 不可依賴 tauri 或 I/O`).not.toMatch(
        /(?:use\s+tauri|std::fs|std::process::Command)/,
      );
    }
  });
});
