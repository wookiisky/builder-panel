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

const forbiddenRuntimeMockApiPath = join(
  repositoryRoot,
  "src",
  "api",
  "mockPanelApi.ts",
);
if (existsSync(forbiddenRuntimeMockApiPath)) {
  reportFailure(
    forbiddenRuntimeMockApiPath,
    "产品运行时不允许保留 mock Tauri API 入口",
  );
}

const tauriLibPath = join(repositoryRoot, "src-tauri", "src", "lib.rs");
if (existsSync(tauriLibPath)) {
  const text = readText(tauriLibPath);
  const forbiddenMockCommands = [
    "get_mock_sessions",
    "get_mock_session_detail",
    "resolve_mock_approval",
    "submit_mock_choice",
    "send_mock_reply",
    "query_mock_timeline",
    "release_mock_timeline_cache",
    "reset_mock_runtime",
  ];
  for (const commandName of forbiddenMockCommands) {
    if (text.includes(commandName)) {
      reportFailure(
        tauriLibPath,
        `产品 Tauri handler 不允许注册 mock command：${commandName}`,
      );
    }
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
