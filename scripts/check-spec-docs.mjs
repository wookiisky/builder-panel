import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const repositoryRoot = process.cwd();
const specRoot = join(repositoryRoot, "spec");
const indexPath = join(specRoot, "00_INDEX.md");
const failures = [];

const collectMarkdownFiles = (directoryPath) => {
  const entries = readdirSync(directoryPath);
  const files = [];

  for (const entry of entries) {
    const entryPath = join(directoryPath, entry);
    const entryStat = statSync(entryPath);

    if (entryStat.isDirectory()) {
      files.push(...collectMarkdownFiles(entryPath));
      continue;
    }

    if (entryPath.endsWith(".md")) {
      files.push(entryPath);
    }
  }

  return files.sort();
};

const fail = (filePath, message) => {
  failures.push(`${relative(repositoryRoot, filePath)}: ${message}`);
};

const readText = (filePath) => readFileSync(filePath, "utf8");

if (!existsSync(indexPath)) {
  failures.push("spec/00_INDEX.md: 文档索引不存在");
} else {
  const indexText = readText(indexPath);
  const specFiles = collectMarkdownFiles(specRoot);

  for (const filePath of specFiles) {
    const text = readText(filePath);
    const relativePath = relative(repositoryRoot, filePath);

    if (
      relativePath !== "spec/00_INDEX.md" &&
      !indexText.includes(relativePath)
    ) {
      fail(filePath, "未登记到 spec/00_INDEX.md");
    }

    if (!/^## (职责|模块职责)$/m.test(text)) {
      fail(filePath, "缺少职责说明");
    }

    if (/```/.test(text)) {
      fail(filePath, "不得包含代码块");
    }

    if (/mermaid/i.test(text)) {
      fail(filePath, "不得包含 mermaid");
    }

    if (/^\|/m.test(text)) {
      fail(filePath, "不得包含 Markdown 表格");
    }

    if (!isSpecialSpecDoc(relativePath) && !/^## 代码入口$/m.test(text)) {
      fail(filePath, "缺少代码入口");
    }

    if (
      !isSpecialSpecDoc(relativePath) &&
      !/^## (相关测试|验收入口)$/m.test(text)
    ) {
      fail(filePath, "缺少测试或验收入口");
    }
  }
}

if (failures.length > 0) {
  console.error("spec 文档质量检查失败：");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("spec 文档质量检查通过。");

function isSpecialSpecDoc(relativePath) {
  return ["spec/00_INDEX.md", "spec/DECISION_LOG.md", "spec/TEST.md"].includes(
    relativePath,
  );
}
