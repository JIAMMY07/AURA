import assert from 'node:assert/strict';
import test from 'node:test';
import worker from './relay-worker.mjs';

const secret = 'aura-pairing-secret-with-at-least-32-characters';
const values = new Map();
const env = {
  AURA_PAIRING_SECRET: secret,
  AURA_COMMANDS: {
    get: async key => values.get(key) ?? null,
    put: async (key, value) => values.set(key, value),
    delete: async key => values.delete(key)
  }
};
const headers = { authorization: `Bearer ${secret}` };
const envelope = { version: 1, id: 'test-id', signature: 'firma' };

test('relay accoda, consegna una volta e poi restituisce 204', async () => {
  values.clear();
  const accepted = await worker.fetch(new Request('https://relay.example/aura/commands', {
    method: 'POST', headers, body: JSON.stringify(envelope)
  }), env);
  assert.equal(accepted.status, 202);
  const delivered = await worker.fetch(new Request('https://relay.example/aura/commands/next', { headers }), env);
  assert.equal(delivered.status, 200);
  assert.deepEqual(await delivered.json(), envelope);
  const empty = await worker.fetch(new Request('https://relay.example/aura/commands/next', { headers }), env);
  assert.equal(empty.status, 204);
});

test('relay rifiuta richieste senza bearer corretto', async () => {
  const response = await worker.fetch(new Request('https://relay.example/aura/commands/next'), env);
  assert.equal(response.status, 401);
});
