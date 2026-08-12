import crypto from 'node:crypto';

const commandForIntent = (intent) => {
  switch (intent?.name) {
    case 'PcStatusIntent': return ['pc.status', {}];
    case 'SecurityStatusIntent': return ['security.status', {}];
    case 'OpenAppIntent': return ['app.open', { application: intent.slots?.application?.value ?? '' }];
    case 'LockPcIntent': return ['system.lock', {}];
    case 'AskAuraIntent': return ['chat.ask', { query: intent.slots?.query?.value ?? '' }];
    default: return undefined;
  }
};

const alexaResponse = (text) => ({
  version: '1.0',
  response: { outputSpeech: { type: 'PlainText', text }, shouldEndSession: true }
});

export const handler = async (event) => {
  if (!process.env.AURA_RELAY_URL || !process.env.AURA_PAIRING_SECRET) {
    throw new Error('Configurazione relay AURA incompleta');
  }
  const relay = new URL(process.env.AURA_RELAY_URL);
  if (relay.protocol !== 'https:' || process.env.AURA_PAIRING_SECRET.length < 32) {
    throw new Error('Relay AURA deve usare HTTPS e un segreto di almeno 32 caratteri');
  }
  const applicationId = event?.context?.System?.application?.applicationId
    ?? event?.session?.application?.applicationId;
  if (!process.env.ALEXA_SKILL_ID || applicationId !== process.env.ALEXA_SKILL_ID) {
    throw new Error('Alexa applicationId non autorizzato');
  }
  if (event?.request?.type === 'LaunchRequest') {
    return alexaResponse('AURA è pronta. Puoi chiedere lo stato del PC.');
  }
  if (event?.request?.type !== 'IntentRequest') {
    return alexaResponse('Richiesta non supportata.');
  }
  const mapped = commandForIntent(event.request.intent);
  if (!mapped) return alexaResponse('Comando non riconosciuto.');
  const [command, payload] = mapped;
  const envelope = {
    version: 1,
    id: crypto.randomUUID(),
    issuedAt: Math.floor(Date.now() / 1000),
    nonce: crypto.randomBytes(24).toString('base64url'),
    command,
    payloadJson: JSON.stringify(payload),
    signature: ''
  };
  const canonical = [envelope.version, envelope.id, envelope.issuedAt, envelope.nonce, envelope.command, envelope.payloadJson].join('\n');
  envelope.signature = crypto.createHmac('sha256', process.env.AURA_PAIRING_SECRET)
    .update(canonical).digest('base64url');
  const response = await fetch(new URL('commands', `${relay.href.replace(/\/$/, '')}/`), {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${process.env.AURA_PAIRING_SECRET}` },
    body: JSON.stringify(envelope),
    signal: AbortSignal.timeout(3_000)
  });
  if (!response.ok) throw new Error(`Relay AURA HTTP ${response.status}`);
  return alexaResponse(command === 'pc.status' ? 'Richiesta inviata ad AURA.' : 'Comando inviato ad AURA; le azioni sensibili richiederanno conferma sul PC.');
};
