const PVGIS = "https://re.jrc.ec.europa.eu/api/tmy";

export default {
  async fetch(request, env) {
    const origin = request.headers.get("Origin") || "";
    const allowed = (env.ALLOWED_ORIGINS || "").split(",").map(x => x.trim()).filter(Boolean);
    if (request.method === "OPTIONS") return new Response(null, { headers: cors(origin, allowed) });
    if (request.method !== "GET") return new Response("GET only", { status: 405 });
    const input = new URL(request.url);
    const lat = Number(input.searchParams.get("lat"));
    const lon = Number(input.searchParams.get("lon"));
    if (!Number.isFinite(lat) || lat < -90 || lat > 90 || !Number.isFinite(lon) || lon < -180 || lon > 180) {
      return new Response("Invalid coordinates", { status: 400, headers: cors(origin, allowed) });
    }
    const target = new URL(PVGIS);
    target.searchParams.set("lat", String(lat)); target.searchParams.set("lon", String(lon)); target.searchParams.set("outputformat", "json");
    const upstream = await fetch(target, { cf: { cacheTtl: 86400, cacheEverything: true } });
    return new Response(upstream.body, { status: upstream.status, headers: { ...cors(origin, allowed), "Content-Type": "application/json", "Cache-Control": "public, max-age=86400" } });
  }
};

function cors(origin, allowed) {
  return { "Access-Control-Allow-Origin": allowed.includes(origin) ? origin : "null", "Access-Control-Allow-Methods": "GET, OPTIONS", "Vary": "Origin" };
}
