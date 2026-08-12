import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import test from 'node:test';
import { handler } from './lambda-handler.mjs';

const applicationId = 'amzn1.ask.skill.aura-test';
const secret = 'aura-pairing-secret-with-at-least-32-characters';

const eventFor = (name, slots = {}) => ({
  context: { System: { application: { applicationId } } },
  request: { type: 'IntentRequest', intent: { name, slots } }
});

test('converte un intento Alexa in una busta AURA autenticata', async () => {
  process.env.ALEXA_SKILL_ID = applicationId;
  process.env.AURA_RELAY_URL = 'https://relay.example.test/aura/';
  process.env.AURA_PAIRING_SECRET = secret;
  let captured;
  globalThis.fetch = async (url, init) => {
    captured = { url: String(url), init };
    return { ok: true, status: 202 };
  };

  const response = await handler(eventFor('PcStatusIntent'));
  assert.equal(response.version, '1.0');
  assert.equal(captured.url, 'https://relay.example.test/aura/commands');
  assert.equal(captured.init.headers.authorization, `Bearer ${secret}`);
  const envelope = JSON.parse(captured.init.body);
  assert.equal(envelope.command, 'pc.status');
  const canonical = [envelope.version, envelope.id, envelope.issuedAt, envelope.nonce, envelope.command, envelope.payloadJson].join('\n');
  const expected = crypto.createHmac('sha256', secret).update(canonical).digest('base64url');
  assert.equal(envelope.signature, expected);
});

test('rifiuta una skill Alexa diversa da quella configurata', async () => {
  process.env.ALEXA_SKILL_ID = applicationId;
  process.env.AURA_RELAY_URL = 'https://relay.example.test/';
  process.env.AURA_PAIRING_SECRET = secret;
  await assert.rejects(
    handler({ ...eventFor('PcStatusIntent'), context: { System: { application: { applicationId: 'altra-skill' } } } }),
    /non autorizzato/
  );
});

test('impone HTTPS e una chiave robusta per il relay', async () => {
  process.env.ALEXA_SKILL_ID = applicationId;
  process.env.AURA_RELAY_URL = 'http://relay.example.test/';
  process.env.AURA_PAIRING_SECRET = 'corta';
  await assert.rejects(handler(eventFor('PcStatusIntent')), /HTTPS/);
});
