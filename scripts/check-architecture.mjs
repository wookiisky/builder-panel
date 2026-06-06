import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

const repositoryRoot = process.cwd();

const failures = [];

const collectFiles = (directoryPath, predicate) => {
  if (!existsSync(directoryPath)) {
    return [];
  }

  const entries = readdirSync(directoryPath);
  const files = [];

  for (const entry of entries) {
    const entryPath = join(directoryPath, entry);
    const entryStat = statSync(entryPath);

    if (entryStat.isDirectory()) {
      files.push(...collectFiles(entryPath, predicate));
      continue;
    }

    if (predicate(entryPath)) {
      files.push(entryPath);
    }
  }

  return files;
};

const reportFailure = (filePath, message) => {
  const displayPath = relative(repositoryRoot, filePath);
  failures.push(`${displayPath}: ${message}`);
};

const readText = (filePath) => readFileSync(filePath, "utf8");

const rustDomainPath = join(repositoryRoot, "src-tauri", "src", "domain");
const rustDomainFiles = collectFiles(rustDomainPath, (filePath) =>
  filePath.endsWith(".rs"),
);
const rustDomainForbiddenPatterns = [
  { pattern: /\btauri\b/, reason: "Domain 不允许依赖 Tauri" },
  { pattern: /\bstd::fs\b/, reason: "Domain 不允许直接读写文件系统" },
  { pattern: /\btokio\b/, reason: "Domain 不允许依赖异步运行时" },
  {
    pattern: /\bserde_json::Value\b/,
    reason: "Domain 不允许接收裸 JSON Value",
  },
  { pattern: /\bcrate::adapters\b/, reason: "Domain 不允许依赖 adapters" },
  { pattern: /\bcrate::services\b/, reason: "Domain 不允许依赖 services" },
  { pattern: /\bcrate::tauri_api\b/, reason: "Domain 不允许依赖 tauri_api" },
];

for (const filePath of rustDomainFiles) {
  const text = readText(filePath);

  for (const forbidden of rustDomainForbiddenPatterns) {
    if (forbidden.pattern.test(text)) {
      reportFailure(filePath, forbidden.reason);
    }
  }
}

const frontendPath = join(repositoryRoot, "src");
const frontendFiles = collectFiles(frontendPath, (filePath) =>
  /\.(ts|tsx)$/.test(filePath),
);

for (const filePath of frontendFiles) {
  const text = readText(filePath);
  const relativeParts = relative(frontendPath, filePath).split(sep);
  const topLevelFolder = relativeParts[0];

  if (
    /(from\s+['"].*src-tauri|from\s+['"].*adapters|import\s+['"].*adapters)/.test(
      text,
    )
  ) {
    reportFailure(filePath, "前端不允许直接引用 Rust 工程或 adapter");
  }

  if (
    topLevelFolder === "api" &&
    /from\s+['"].*\.\.\/(components|views|stores)\//.test(text)
  ) {
    reportFailure(filePath, "前端 api 层不允许反向依赖 UI 或 store");
  }
}

if (failures.length > 0) {
  console.error("架构边界检查失败：");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("架构边界检查通过。");
