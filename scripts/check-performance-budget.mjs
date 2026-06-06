const SESSION_COUNT = 10;
const EVENT_COUNT = 1_000;
const TIMELINE_COUNT = 10_000;
const SESSION_LIMIT = 500;
const LARGE_TEXT_THRESHOLD = 512;

const sessions = Array.from({ length: SESSION_COUNT }, (_, index) => ({
  sessionKey: `mock/project-${index}/conversation-${index}`,
  status: index % 3 === 0 ? "waiting" : "running",
  updatedAt: index,
}));

const events = Array.from({ length: EVENT_COUNT }, (_, index) => ({
  id: `event-${index}`,
  sessionKey: sessions[index % sessions.length].sessionKey,
  kind: index % 10 === 0 ? "approval" : "activity",
  body: `mock event ${index}`,
}));

const timeline = [];
for (let index = 0; index < TIMELINE_COUNT; index += 1) {
  timeline.push({
    id: `timeline-${index}`,
    sessionKey: sessions[index % sessions.length].sessionKey,
    priority: index % 100 === 0 ? "high" : "low",
    body: index % 250 === 0 ? "长正文".repeat(300) : `短正文 ${index}`,
  });
}

const visibleRange = calculateVisibleRange({
  scrollTop: 720,
  rowHeight: 36,
  viewportHeight: 360,
  totalCount: timeline.length,
  overscan: 6,
});
const retained = enforceSessionLimit(timeline, SESSION_LIMIT);
const released = releaseLargeTexts(retained);

assert(sessions.length === SESSION_COUNT, "10 session 场景生成失败");
assert(events.length === EVENT_COUNT, "1000 event 场景生成失败");
assert(timeline.length === TIMELINE_COUNT, "1 万 timeline 场景生成失败");
assert(
  visibleRange.end - visibleRange.start < 40,
  "虚拟列表可见范围不应接近全量记录",
);
assert(
  retained.every((items) => items.length <= SESSION_LIMIT),
  "timeline 单 session 淘汰策略未生效",
);
assert(released > 0, "大文本缓存释放场景未生效");

console.log("性能预算静态场景检查通过。");

function calculateVisibleRange({
  scrollTop,
  rowHeight,
  viewportHeight,
  totalCount,
  overscan,
}) {
  const firstVisible = Math.floor(scrollTop / rowHeight);
  const visibleCount = Math.ceil(viewportHeight / rowHeight);
  const start = Math.max(0, firstVisible - overscan);
  const end = Math.min(totalCount, firstVisible + visibleCount + overscan);

  return { start, end };
}

function enforceSessionLimit(items, sessionLimit) {
  const grouped = new Map();

  for (const item of items) {
    const sessionItems = grouped.get(item.sessionKey) ?? [];
    sessionItems.push(item);
    grouped.set(item.sessionKey, sessionItems);
  }

  for (const [sessionKey, sessionItems] of grouped) {
    while (sessionItems.length > sessionLimit) {
      const lowPriorityIndex = sessionItems.findIndex(
        (item) => item.priority === "low",
      );
      sessionItems.splice(lowPriorityIndex === -1 ? 0 : lowPriorityIndex, 1);
    }
    grouped.set(sessionKey, sessionItems);
  }

  return Array.from(grouped.values());
}

function releaseLargeTexts(groupedItems) {
  let released = 0;

  for (const items of groupedItems) {
    for (const item of items) {
      if (item.body.length <= LARGE_TEXT_THRESHOLD) {
        continue;
      }

      item.body = "长正文缓存已释放";
      released += 1;
    }
  }

  return released;
}

function assert(condition, message) {
  if (condition) {
    return;
  }

  console.error(`性能预算检查失败：${message}`);
  process.exit(1);
}
