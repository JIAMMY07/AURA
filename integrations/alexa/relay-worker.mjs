const json = (body, status = 200) => new Response(JSON.stringify(body), {
  status,
  headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' }
});

function authorized(request, secret) {
  const supplied = request.headers.get('authorization') ?? '';
  const expected = `Bearer ${secret}`;
  if (supplied.length !== expected.length) return false;
  let difference = 0;
  for (let index = 0; index < expected.length; index += 1) {
    difference |= supplied.charCodeAt(index) ^ expected.charCodeAt(index);
  }
  return difference === 0;
}

export default {
  async fetch(request, env) {
    if (!env.AURA_PAIRING_SECRET || env.AURA_PAIRING_SECRET.length < 32 || !env.AURA_COMMANDS) {
      return json({ error: 'relay non configurato' }, 503);
    }
    if (!authorized(request, env.AURA_PAIRING_SECRET)) return json({ error: 'non autorizzato' }, 401);
    const url = new URL(request.url);
    if (url.pathname.endsWith('/commands') && request.method === 'POST') {
      const raw = await request.text();
      if (new TextEncoder().encode(raw).length > 16 * 1024) return json({ error: 'busta troppo grande' }, 413);
      let envelope;
      try { envelope = JSON.parse(raw); } catch { return json({ error: 'JSON non valido' }, 400); }
      if (envelope?.version !== 1 || typeof envelope?.id !== 'string' || typeof envelope?.signature !== 'string') {
        return json({ error: 'busta non valida' }, 400);
      }
      const pending = await env.AURA_COMMANDS.get('pending');
      if (pending) return json({ error: 'coda occupata; riprova' }, 409);
      await env.AURA_COMMANDS.put('pending', raw, { expirationTtl: 120 });
      return json({ accepted: true, id: envelope.id }, 202);
    }
    if (url.pathname.endsWith('/commands/next') && request.method === 'GET') {
      const pending = await env.AURA_COMMANDS.get('pending');
      if (!pending) return new Response(null, { status: 204, headers: { 'cache-control': 'no-store' } });
      await env.AURA_COMMANDS.delete('pending');
      return new Response(pending, { headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' } });
    }
    return json({ error: 'endpoint non trovato' }, 404);
  }
};
