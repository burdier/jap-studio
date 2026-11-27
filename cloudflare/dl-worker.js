const BOT_RE =
  /bot|crawl|spider|slurp|facebookexternalhit|twitterbot|whatsapp|telegrambot|discordbot|preview|uptime|statuscake|pingdom|headless|lighthouse|ahrefs|semrush|mj12bot|bytespider|petalbot/i;

const PACKAGE_EXTS = [
  { ext: ".flatpak", platform: "linux", mime: "application/vnd.flatpak" },
  { ext: ".tar.xz", platform: "linux", mime: "application/x-xz" },
  { ext: ".appimage", platform: "linux", mime: "application/octet-stream" },
  { ext: ".zip", platform: "windows", mime: "application/zip" },
  { ext: ".exe", platform: "windows", mime: "application/vnd.microsoft.portable-executable" },
  { ext: ".msi", platform: "windows", mime: "application/x-msi" },
  { ext: ".dmg", platform: "macos", mime: "application/x-apple-diskimage" },
  { ext: ".pkg", platform: "macos", mime: "application/octet-stream" },
];

function classifyFile(key) {
  const lower = key.toLowerCase();
  const match = PACKAGE_EXTS.find((entry) => lower.endsWith(entry.ext));
  const firstSeg = key.split("/")[0] || "unknown";
  return {
    isPackage: Boolean(match),
    platform: match ? match.platform : "other",
    fallbackMime: match ? match.mime : "application/octet-stream",
    version: firstSeg,
    filename: key.split("/").pop() || "download",
  };
}

function isBot(request) {
  const ua = request.headers.get("user-agent") || "";
  return BOT_RE.test(ua);
}

function parseRangeHeader(header, size) {
  if (!header || !header.toLowerCase().startsWith("bytes=")) return null;
  const spec = header.slice(6).split(",")[0].trim();
  const dash = spec.indexOf("-");
  if (dash === -1) return { invalid: true };

  const startStr = spec.slice(0, dash).trim();
  const endStr = spec.slice(dash + 1).trim();

  if (startStr === "" && endStr === "") return { invalid: true };

  if (startStr === "") {
    const suffix = Number(endStr);
    if (!Number.isInteger(suffix) || suffix <= 0) return { invalid: true };
    const length = Math.min(suffix, size);
    const offset = size - length;
    return { offset, end: size - 1, length };
  }

  const offset = Number(startStr);
  if (!Number.isInteger(offset) || offset < 0 || offset >= size) {
    return { invalid: true };
  }

  let end = size - 1;
  if (endStr !== "") {
    const parsedEnd = Number(endStr);
    if (!Number.isInteger(parsedEnd) || parsedEnd < offset) {
      return { invalid: true };
    }
    end = Math.min(parsedEnd, size - 1);
  }

  return { offset, end, length: end - offset + 1 };
}

function cacheControlFor(key) {
  return key.startsWith("latest/")
    ? "public, max-age=300, must-revalidate"
    : "public, max-age=86400";
}

function etagMatches(ifNoneMatch, httpEtag) {
  if (!ifNoneMatch || !httpEtag) return false;
  if (ifNoneMatch.trim() === "*") return true;
  const normalize = (v) => v.trim().replace(/^W\//, "");
  const target = normalize(httpEtag);
  return ifNoneMatch.split(",").some((part) => normalize(part) === target);
}

function buildFileHeaders(obj, key, meta, rangeInfo) {
  const headers = new Headers();
  obj.writeHttpMetadata(headers);
  if (!headers.has("content-type")) {
    headers.set("content-type", meta.fallbackMime);
  }
  headers.set("etag", obj.httpEtag);
  headers.set("accept-ranges", "bytes");
  headers.set("cache-control", cacheControlFor(key));
  headers.set("x-content-type-options", "nosniff");
  headers.set("referrer-policy", "strict-origin-when-cross-origin");

  const safeName = meta.filename.replace(/["\\\r\n]/g, "_");
  headers.set("content-disposition", `attachment; filename="${safeName}"`);

  if (rangeInfo) {
    headers.set("content-range", `bytes ${rangeInfo.offset}-${rangeInfo.end}/${obj.size}`);
    headers.set("content-length", String(rangeInfo.length));
  } else {
    headers.set("content-length", String(obj.size));
  }
  return headers;
}

async function recordDownload(db, key, meta) {
  try {
    await db.batch([
      db
        .prepare(
          `INSERT INTO downloads (path, platform, version, count, last_download_at)
           VALUES (?1, ?2, ?3, 1, datetime('now'))
           ON CONFLICT(path) DO UPDATE SET
             count = count + 1,
             last_download_at = datetime('now')`
        )
        .bind(key, meta.platform, meta.version),
      db
        .prepare(
          `INSERT INTO daily_downloads (day, platform, count)
           VALUES (date('now'), ?1, 1)
           ON CONFLICT(day, platform) DO UPDATE SET
             count = count + 1`
        )
        .bind(meta.platform),
    ]);
  } catch (err) {
    console.error("Failed to record download", { key, error: String(err) });
  }
}

async function handleStats(db, method) {
  const corsHeaders = {
    "content-type": "application/json; charset=utf-8",
    "cache-control": "public, max-age=30, stale-while-revalidate=60",
    "access-control-allow-origin": "*",
    "access-control-allow-methods": "GET, HEAD, OPTIONS",
    "x-content-type-options": "nosniff",
  };

  if (method === "OPTIONS") {
    return new Response(null, { status: 204, headers: corsHeaders });
  }

  const [filesRes, dailyRes] = await db.batch([
    db.prepare(
      `SELECT path, platform, version, count, last_download_at
       FROM downloads
       ORDER BY count DESC, path ASC`
    ),
    db.prepare(
      `SELECT day, platform, count
       FROM daily_downloads
       WHERE day >= date('now', '-30 days')
       ORDER BY day DESC, platform ASC`
    ),
  ]);

  const files = filesRes.results || [];
  let total = 0;
  const byPlatform = { linux: 0, windows: 0 };
  const byVersion = {};

  for (const row of files) {
    const c = Number(row.count) || 0;
    total += c;
    byPlatform[row.platform] = (byPlatform[row.platform] || 0) + c;
    byVersion[row.version] = (byVersion[row.version] || 0) + c;
  }

  const dailyMap = new Map();
  for (const row of dailyRes.results || []) {
    if (!dailyMap.has(row.day)) {
      dailyMap.set(row.day, { day: row.day, total: 0, linux: 0, windows: 0 });
    }
    const entry = dailyMap.get(row.day);
    const c = Number(row.count) || 0;
    entry.total += c;
    entry[row.platform] = (entry[row.platform] || 0) + c;
  }

  const payload = {
    total,
    by_platform: byPlatform,
    by_version: byVersion,
    files,
    daily_last_30d: Array.from(dailyMap.values()),
    updated_at: new Date().toISOString(),
  };

  const body = method === "HEAD" ? null : JSON.stringify(payload, null, 2);
  return new Response(body, { status: 200, headers: corsHeaders });
}

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    let key;
    try {
      key = decodeURIComponent(url.pathname).replace(/^\/+/, "");
    } catch {
      return new Response("Bad Request", { status: 400 });
    }

    if (key === "") {
      return Response.redirect("https://jap.sevenpitch.dev/#descargar", 302);
    }

    if (key === "stats" || key === "stats.json") {
      if (!["GET", "HEAD", "OPTIONS"].includes(request.method)) {
        return new Response("Method Not Allowed", {
          status: 405,
          headers: { Allow: "GET, HEAD, OPTIONS" },
        });
      }
      return handleStats(env.DB, request.method);
    }

    if (key.includes("..") || key.includes("\\")) {
      return new Response("Bad Request", { status: 400 });
    }

    if (request.method === "OPTIONS") {
      return new Response(null, {
        status: 204,
        headers: {
          Allow: "GET, HEAD, OPTIONS",
          "Access-Control-Allow-Origin": "*",
        },
      });
    }

    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response("Method Not Allowed", {
        status: 405,
        headers: { Allow: "GET, HEAD, OPTIONS" },
      });
    }

    const meta = classifyFile(key);

    if (request.method === "HEAD") {
      const head = await env.BUCKET.head(key);
      if (!head) return new Response("Not Found", { status: 404 });
      const headers = buildFileHeaders(head, key, meta, null);
      return new Response(null, { status: 200, headers });
    }

    const rangeHeader = request.headers.get("range");
    const ifNoneMatch = request.headers.get("if-none-match");

    if (rangeHeader) {
      const head = await env.BUCKET.head(key);
      if (!head) return new Response("Not Found", { status: 404 });

      if (etagMatches(ifNoneMatch, head.httpEtag)) {
        return new Response(null, {
          status: 304,
          headers: {
            etag: head.httpEtag,
            "cache-control": cacheControlFor(key),
          },
        });
      }

      const rangeInfo = parseRangeHeader(rangeHeader, head.size);
      if (!rangeInfo || rangeInfo.invalid) {
        return new Response("Range Not Satisfiable", {
          status: 416,
          headers: { "content-range": `bytes */${head.size}` },
        });
      }

      const obj = await env.BUCKET.get(key, {
        range: { offset: rangeInfo.offset, length: rangeInfo.length },
      });
      if (!obj || !obj.body) return new Response("Not Found", { status: 404 });

      const headers = buildFileHeaders(obj, key, meta, rangeInfo);

      const isInitialRealChunk =
        rangeInfo.offset === 0 &&
        (rangeInfo.end === head.size - 1 || rangeInfo.length >= 65536);

      if (meta.isPackage && isInitialRealChunk && !isBot(request)) {
        ctx.waitUntil(recordDownload(env.DB, key, meta));
      }

      return new Response(obj.body, { status: 206, headers });
    }

    const obj = await env.BUCKET.get(key, { onlyIf: request.headers });
    if (!obj) return new Response("Not Found", { status: 404 });

    if (!("body" in obj) || obj.body === null) {
      return new Response(null, {
        status: 304,
        headers: {
          etag: obj.httpEtag,
          "cache-control": cacheControlFor(key),
        },
      });
    }

    const headers = buildFileHeaders(obj, key, meta, null);

    if (meta.isPackage && !isBot(request)) {
      ctx.waitUntil(recordDownload(env.DB, key, meta));
    }

    return new Response(obj.body, { status: 200, headers });
  },
};
