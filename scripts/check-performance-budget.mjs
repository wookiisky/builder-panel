const SESSION_COUNT = 1_000;
const EVENT_COUNT = 1_000;
const SUMMARY_LIMIT = 96;

const sessions = Array.from({ length: SESSION_COUNT }, (_, index) => ({
  sessionKey: `mock/project-${index % 25}/conversation-${index}`,
  status:
    index % 5 === 0
      ? "completed"
      : index % 7 === 0
        ? "failed"
        : index % 3 === 0
          ? "waiting"
          : "running",
  summary: `mock summary ${index} ${"正文".repeat(80)}`,
  captureSequence: index,
}));

const events = Array.from({ length: EVENT_COUNT }, (_, index) => ({
  id: `event-${index}`,
  sessionKey: sessions[index % sessions.length].sessionKey,
  updatedAt: index,
}));

const refreshed = sessions.map((session, index) => ({
  ...session,
  updatedAt: events[index % events.length].updatedAt,
}));
const merged = mergeByCaptureOrder(sessions, refreshed);
const truncated = merged.map((session) =>
  truncateSummary(session.summary, SUMMARY_LIMIT),
);
const expanded = toggleExpansionForCompletedSessions(merged);

assert(sessions.length === SESSION_COUNT, "1000 session 场景生成失败");
assert(events.length === EVENT_COUNT, "1000 event 场景生成失败");
assert(merged.length === SESSION_COUNT, "session 捕捉顺序合并后数量不稳定");
assert(
  merged.every((session, index) => session.captureSequence === index),
  "session 捕捉顺序合并不稳定",
);
assert(
  truncated.every((summary) => Array.from(summary).length <= SUMMARY_LIMIT),
  "摘要截断预算未生效",
);
assert(expanded.size > 0, "follow-up 展开集合场景未生效");
assert(
  expanded.size < SESSION_COUNT,
  "follow-up 展开集合不应接近全量 session",
);

console.log("性能预算静态场景检查通过。");

function mergeByCaptureOrder(previousSessions, nextSessions) {
  const nextById = new Map(
    nextSessions.map((session) => [session.sessionKey, session]),
  );

  return previousSessions
    .map((session) => nextById.get(session.sessionKey) ?? null)
    .filter((session) => session !== null);
}

function truncateSummary(summary, maxChars) {
  return Array.from(summary).slice(0, maxChars).join("");
}

function toggleExpansionForCompletedSessions(items) {
  const expanded = new Set();

  for (const item of items) {
    if (item.status !== "completed" && item.status !== "failed") {
      continue;
    }
    if (expanded.has(item.sessionKey)) {
      expanded.delete(item.sessionKey);
    } else {
      expanded.add(item.sessionKey);
    }
  }

  return expanded;
}

function assert(condition, message) {
  if (condition) {
    return;
  }

  console.error(`性能预算检查失败：${message}`);
  process.exit(1);
}
