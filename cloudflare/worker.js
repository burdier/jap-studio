export default {
  async fetch(request, env) {
    const res = await env.ASSETS.fetch(request);
    const headers = new Headers(res.headers);
    const path = new URL(request.url).pathname;
    headers.set("X-Content-Type-Options", "nosniff");
    headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
    if (path.endsWith(".mp4") || path.endsWith(".png") || path.endsWith(".svg") || path.endsWith(".ico")) {
      headers.set("Cache-Control", "public, max-age=86400, stale-while-revalidate=604800");
    } else if (path.endsWith(".pdf")) {
      headers.set("Cache-Control", "public, max-age=3600, stale-while-revalidate=86400");
    } else if (path === "/" || path.endsWith(".html")) {
      headers.set("Cache-Control", "public, max-age=300, stale-while-revalidate=3600");
    }
    return new Response(res.body, { status: res.status, statusText: res.statusText, headers });
  }
};
