import { expect, test } from '@playwright/test';

test('carica il control center e rende accessibili i moduli principali', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  await expect(page.getByText('AURA', { exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Buonasera. Cosa vuoi fare?' })).toBeVisible();
  await expect(page.getByText('Browser autonomo controllato')).toBeVisible();
  await expect(page.getByText('Connettori API REST')).toBeVisible();
  await expect(page.getByText('RAG documentale locale')).toBeVisible();
  await expect(page.locator('#security-section > summary')).toContainText('Sicurezza');
});

test('arresto globale è sempre disponibile e sblocca l’interfaccia', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  const stop = page.getByRole('button', { name: 'Arresta tutto' });
  await expect(stop).toBeVisible();
  await stop.click();
  await expect(page.getByText('Interfaccia sbloccata', { exact: false })).toBeVisible();
});

test('la barra laterale maggiorata si nasconde e riapre come banner', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  const sidebar = page.getByRole('complementary', { name: 'Barra laterale AURA' });
  const box = await sidebar.boundingBox();
  expect(box?.width).toBeGreaterThanOrEqual(279);
  const navFont = await sidebar.getByRole('button', { name: 'Assistente' }).evaluate(element => getComputedStyle(element).fontSize);
  expect(Number.parseFloat(navFont)).toBeGreaterThanOrEqual(17);
  await sidebar.getByRole('button', { name: 'Nascondi barra laterale' }).click();
  await expect(page.getByRole('button', { name: 'Mostra barra laterale' })).toBeVisible();
  await page.getByRole('button', { name: 'Mostra barra laterale' }).click();
  await expect(sidebar).toBeVisible();
});

test('il banner Accesso PC espone profili, rete e approvazioni', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  const banner = page.getByRole('region', { name: 'Accesso PC e approvazioni' });
  await expect(banner.locator('.pc-access-summary strong')).toHaveText('Controllato');
  await expect(banner.getByText('Confine', { exact: true })).toBeVisible();
  await expect(banner.getByText('Rete', { exact: true })).toBeVisible();
  await expect(banner.getByText('Approvazioni', { exact: true })).toBeVisible();
  await banner.getByRole('combobox', { name: 'Modalità accesso PC' }).selectOption('read-only');
  await expect(banner.locator('.pc-access-summary strong')).toHaveText('Sola lettura');
  await banner.getByRole('button', { name: 'Nascondi dettagli accesso PC' }).click();
  await expect(banner.getByText('Confine', { exact: true })).toHaveCount(0);
  await expect(banner.getByRole('button', { name: 'Mostra dettagli accesso PC' })).toBeVisible();
});

test('carica i modelli locali e controlla esplicitamente la videocamera', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  const vision = page.getByRole('region', { name: 'Riconoscimento locale di mani e volto' });
  await expect(vision.getByText('SPENTA', { exact: true })).toBeVisible();
  await expect(vision.getByText('La videocamera è spenta')).toBeVisible();
  await vision.getByRole('button', { name: 'Avvia' }).click();
  await expect(vision.getByText('ATTIVA', { exact: true })).toBeVisible({ timeout: 20_000 });
  await expect(vision.getByText('Nessuna immagine viene salvata', { exact: false })).toBeVisible();
  const handControl = vision.getByRole('checkbox', { name: /Scorciatoie multimediali/ });
  await handControl.check();
  await expect(vision.getByText('Controllo mani armato', { exact: false })).toBeVisible();
  await vision.getByRole('button', { name: 'Riduci videocamera a banner' }).click();
  await expect(vision.getByLabel('Anteprima videocamera locale')).toHaveCount(0);
  await vision.getByRole('button', { name: 'Mostra pannello videocamera' }).click();
  await vision.getByRole('button', { name: 'Arresta' }).click();
  await expect(vision.getByText('SPENTA', { exact: true })).toBeVisible();
});

test('policy critica resta confinata in sandbox e la UI non deborda', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  await page.locator('#activity-section > summary').click();
  await page.getByText('Laboratorio policy').click();
  await page.getByRole('button', { name: 'Installa un nuovo modulo' }).click();
  await expect(page.getByText('SANDBOX', { exact: true })).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
  expect(overflow).toBe(false);
});

test('i pannelli modulari espongono i controlli previsti', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  const navigation = page.getByRole('navigation', { name: 'Sezioni principali' });
  await navigation.getByRole('button', { name: 'Sicurezza' }).click();
  await page.getByText('Security Center AURA').click();
  await expect(page.getByRole('button', { name: 'Aggiorna controlli' })).toBeVisible();
  await page.getByText('Browser autonomo controllato').click();
  await expect(page.getByRole('button', { name: 'Apri e prendi controllo' })).toBeVisible();
  await page.getByText('Connettori API REST').click();
  await expect(page.getByRole('button', { name: 'Salva cifrato' })).toBeVisible();
  await navigation.getByRole('button', { name: 'Attività' }).click();
  await page.getByText('Log e diagnostica AI').click();
  await expect(page.getByRole('button', { name: 'Scarica JSON' })).toBeVisible();
  await expect(page.getByText('Le interazioni voce, Alexa e gesti sono cifrate', { exact: false })).toBeVisible();
  await expect(page.getByLabel('Origine')).toBeVisible();
  await page.getByText('Workspace, file e workflow').click();
  await expect(page.getByRole('textbox', { name: 'Percorso workspace da autorizzare' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Autorizza cartella' })).toBeVisible();
  await page.getByText('Applicazioni installate').click();
  await expect(page.getByRole('textbox', { name: 'Cerca applicazione installata' })).toBeVisible();
  await page.getByText('Evolution Lab', { exact: true }).click();
  await expect(page.getByText('Attestazione v2', { exact: false })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Genera patch e prepara revisione' })).toBeVisible();
});

test('attività e sicurezza sono banner indipendenti con sottosezioni richiudibili', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  const activity = page.locator('#activity-section');
  const security = page.locator('#security-section');
  await expect(activity).toHaveJSProperty('open', false);
  await expect(security).toHaveJSProperty('open', false);

  await activity.locator(':scope > summary').click();
  await expect(activity).toHaveJSProperty('open', true);
  const audit = activity.getByText('Log e diagnostica AI', { exact: true });
  await audit.click();
  await expect(activity.locator('.audit-log-panel')).toHaveJSProperty('open', true);
  await audit.click();
  await expect(activity.locator('.audit-log-panel')).toHaveJSProperty('open', false);

  await security.locator(':scope > summary').click();
  await expect(security).toHaveJSProperty('open', true);
  await security.getByText('Privacy e cifratura verificabili', { exact: true }).click();
  await expect(security.locator('details').filter({ hasText: 'Privacy e cifratura verificabili' })).toHaveJSProperty('open', true);
  await expect(activity).toHaveJSProperty('open', true);
});

test('la navigazione laterale apre attività e sicurezza', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  const navigation = page.getByRole('navigation', { name: 'Sezioni principali' });

  await navigation.getByRole('button', { name: 'Attività' }).click();
  await expect(navigation.getByRole('button', { name: 'Attività' })).toHaveAttribute('aria-current', 'page');
  await expect(page.locator('#activity-section')).toHaveJSProperty('open', true);

  await navigation.getByRole('button', { name: 'Sicurezza' }).click();
  await expect(navigation.getByRole('button', { name: 'Sicurezza' })).toHaveAttribute('aria-current', 'page');
  await expect(page.locator('#security-section')).toHaveJSProperty('open', true);

  await navigation.getByRole('button', { name: 'Assistente' }).click();
  await expect(navigation.getByRole('button', { name: 'Assistente' })).toHaveAttribute('aria-current', 'page');
});

test('il browser controllato espone stato, indirizzo e ispezione guidata', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  await page.getByText('Browser autonomo controllato').click();
  await expect(page.getByText('Finestra chiusa')).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'URL o ricerca browser' })).toHaveValue('https://example.com');
  await expect(page.getByRole('button', { name: 'Leggi senza aprire' })).toBeEnabled();
  await expect(page.getByRole('button', { name: 'Apri e prendi controllo' })).toBeEnabled();
  await expect(page.getByRole('button', { name: 'Ispeziona finestra' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Aggiorna stato' })).toBeEnabled();
  await expect(page.getByRole('textbox', { name: 'Obiettivo browser autonomo' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Esegui obiettivo' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Prendi controllo ora' })).toBeDisabled();
  await expect(page.getByRole('textbox', { name: 'URL download in quarantena' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Scarica in quarantena' })).toBeDisabled();
});

test('risponde immediatamente alla richiesta sulle capacità', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  await page.getByPlaceholder('Scrivi un comando o una domanda…').fill('cosa sai fare?');
  await page.getByRole('button', { name: 'Invia' }).click();
  await expect(page.getByText('Posso aiutarti con:', { exact: false })).toBeVisible();
  await expect(page.getByText('chat e ragionamento usando modelli Ollama locali', { exact: false })).toBeVisible();
  await expect(page.getByText('non ti do automaticamente ragione', { exact: false })).toBeVisible();
});

test('dichiara correttamente generazione ed esecuzione del codice', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  await page.getByPlaceholder('Scrivi un comando o una domanda…').fill('puoi generare codice?');
  await page.getByRole('button', { name: 'Invia' }).click();
  await expect(page.getByText('Posso generare, spiegare, correggere e revisionare codice', { exact: false })).toBeVisible();
  await expect(page.getByText('L’esecuzione reale non è illimitata', { exact: false })).toBeVisible();
});

test('crea progetti, chat multiple ed esporta la conversazione in PDF', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  const workspace = page.getByRole('region', { name: 'Progetti e conversazioni' });

  page.once('dialog', dialog => dialog.accept('Progetto E2E'));
  await workspace.getByRole('button', { name: 'Nuovo progetto' }).click();
  await expect(workspace.getByRole('button', { name: 'Progetto E2E' })).toBeVisible();

  await page.getByPlaceholder(/Scrivi un comando o una domanda/).fill('cosa sai fare?');
  await page.getByRole('button', { name: 'Invia' }).click();
  await expect(workspace.getByRole('button', { name: 'cosa sai fare?', exact: true })).toBeVisible();

  const downloadPromise = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Esporta chat in PDF' }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toMatch(/\.pdf$/i);
});

test('cerca le chat per nome e cancella soltanto quella selezionata', async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  const workspace = page.getByRole('region', { name: 'Progetti e conversazioni' });
  const composer = page.getByPlaceholder(/Scrivi un comando o una domanda/);

  await workspace.getByRole('button', { name: 'Nuova chat' }).click();
  await composer.fill('cosa sai fare?');
  await page.getByRole('button', { name: 'Invia', exact: true }).click();
  await expect(workspace.getByRole('button', { name: 'cosa sai fare?', exact: true })).toBeVisible();

  await workspace.getByRole('button', { name: 'Nuova chat' }).click();
  await composer.fill('puoi generare codice?');
  await page.getByRole('button', { name: 'Invia', exact: true }).click();
  await expect(workspace.getByRole('button', { name: 'puoi generare codice?', exact: true })).toBeVisible();

  const search = workspace.getByRole('searchbox', { name: 'Cerca chat per nome' });
  await search.fill('GENERARE');
  await expect(workspace.getByRole('button', { name: 'puoi generare codice?', exact: true })).toBeVisible();
  await expect(workspace.getByRole('button', { name: 'cosa sai fare?', exact: true })).toHaveCount(0);

  await search.fill('');
  page.once('dialog', dialog => dialog.accept());
  await workspace.getByRole('button', { name: 'Elimina chat cosa sai fare?' }).click();
  await expect(workspace.getByRole('button', { name: 'cosa sai fare?', exact: true })).toHaveCount(0);
  await expect(workspace.getByRole('button', { name: 'puoi generare codice?', exact: true })).toBeVisible();
});

test('accoda, modifica, elimina e prioritizza domande durante una risposta', async ({ page }) => {
  const prompts: string[] = [];
  await page.route('http://127.0.0.1:11434/api/tags', route => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({ models: [{ name: 'aura:4b', model: 'aura:4b', size: 2_500_000_000, modified_at: 'now', capabilities: ['completion'], details: { format: 'gguf' } }] })
  }));
  await page.route('http://127.0.0.1:11434/api/chat', async route => {
    const request = route.request().postDataJSON() as { messages?: Array<{ role: string; content: string }> };
    prompts.push(request.messages?.at(-1)?.content ?? '');
    await new Promise(resolve => setTimeout(resolve, 1_500));
    await route.fulfill({
      contentType: 'application/x-ndjson',
      body: `${JSON.stringify({ message: { content: 'Risposta simulata' }, done: false })}\n${JSON.stringify({ done: true, eval_count: 2, eval_duration: 1_000_000_000 })}\n`
    }).catch(() => undefined);
  });

  await page.goto('/', { waitUntil: 'domcontentloaded' });
  await expect(page.getByText('1 modelli locali')).toBeVisible();
  const composer = page.getByPlaceholder('Scrivi un comando o una domanda…');
  await composer.fill('prima domanda non locale');
  await page.getByRole('button', { name: 'Invia', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Stop' })).toBeVisible();

  await composer.fill('seconda domanda');
  await page.getByRole('button', { name: 'Accoda' }).click();
  await composer.fill('terza domanda');
  await page.getByRole('button', { name: 'Accoda' }).click();
  await expect(page.getByText('Coda · 2/20')).toBeVisible();

  const second = page.getByRole('textbox', { name: 'Domanda in coda 1' });
  await second.fill('seconda domanda modificata');
  const thirdRow = page.getByRole('textbox', { name: 'Domanda in coda 2' }).locator('..');
  await thirdRow.getByRole('button', { name: 'Elimina' }).click();
  await expect(page.getByText('Coda · 1/20')).toBeVisible();

  await page.getByRole('button', { name: 'Invia ora' }).click();
  await expect.poll(() => prompts).toContain('seconda domanda modificata');
  await expect(page.getByText('Domanda prelevata dalla coda', { exact: false })).toBeVisible();
});
