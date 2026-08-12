import { invoke, isTauri } from '@tauri-apps/api/core';
import type { ChatMessage } from '../adapters/ollama';
import notoSansDataUrl from '../../assets/fonts/NotoSans-Regular.ttf?inline';

export interface ConversationPdfInput {
  projectName: string;
  conversationTitle: string;
  messages: ChatMessage[];
  modelLabel?: string;
}

const PAGE_WIDTH = 210;
const PAGE_HEIGHT = 297;
const MARGIN = 18;
const BODY_WIDTH = PAGE_WIDTH - MARGIN * 2;

function printable(value: string): string {
  return value.normalize('NFC')
    .replace(/[\u2010-\u2015]/g, '-')
    .replace(/\u00a0/g, ' ')
    .replace(/[\u{1F000}-\u{1FAFF}]/gu, '[simbolo]');
}

function safeFileName(value: string): string {
  const safe = printable(value).replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_').trim().slice(0, 70);
  return `${safe || 'Conversazione AURA'}.pdf`;
}

export async function createConversationPdf(input: ConversationPdfInput): Promise<Uint8Array> {
  const { jsPDF } = await import('jspdf');
  const doc = new jsPDF({ unit: 'mm', format: 'a4', compress: true, putOnlyUsedFonts: true });
  const notoSansBase64 = notoSansDataUrl.slice(notoSansDataUrl.indexOf(',') + 1);
  doc.addFileToVFS('NotoSans-Regular.ttf', notoSansBase64);
  doc.addFont('NotoSans-Regular.ttf', 'NotoSans', 'normal');
  doc.addFont('NotoSans-Regular.ttf', 'NotoSans', 'bold');
  doc.setProperties({
    title: printable(input.conversationTitle),
    subject: `Conversazione AURA - ${printable(input.projectName)}`,
    author: 'AURA',
    creator: 'AURA Local AI Assistant'
  });

  let y = 21;
  const addPage = () => {
    doc.addPage();
    y = 21;
  };
  const ensureSpace = (height: number) => {
    if (y + height > PAGE_HEIGHT - 20) addPage();
  };

  doc.setFillColor(10, 18, 32);
  doc.rect(0, 0, PAGE_WIDTH, 42, 'F');
  doc.setTextColor(117, 239, 208);
  doc.setFont('NotoSans', 'bold');
  doc.setFontSize(11);
  doc.text('AURA - CONVERSAZIONE LOCALE', MARGIN, 15);
  doc.setTextColor(239, 244, 255);
  doc.setFontSize(19);
  doc.text(doc.splitTextToSize(printable(input.conversationTitle), BODY_WIDTH), MARGIN, 26);
  y = 51;

  doc.setTextColor(72, 84, 105);
  doc.setFont('NotoSans', 'normal');
  doc.setFontSize(9);
  doc.text(`Progetto: ${printable(input.projectName)}`, MARGIN, y);
  y += 5;
  doc.text(`Esportata: ${new Date().toLocaleString('it-IT')}${input.modelLabel ? ` - Modello: ${printable(input.modelLabel)}` : ''}`, MARGIN, y);
  y += 10;

  if (!input.messages.length) {
    doc.setTextColor(90, 101, 119);
    doc.setFontSize(11);
    doc.text('Questa conversazione non contiene ancora messaggi.', MARGIN, y);
  }

  for (const message of input.messages) {
    const role = message.role === 'user' ? 'TU' : message.role === 'assistant' ? 'AURA' : 'SISTEMA';
    const timestamp = new Date(message.createdAt);
    const date = Number.isNaN(timestamp.getTime()) ? '' : timestamp.toLocaleString('it-IT');
    const lines = doc.splitTextToSize(printable(message.content || ''), BODY_WIDTH - 8) as string[];
    const blockHeight = 12 + Math.max(1, lines.length) * 5;
    ensureSpace(Math.min(blockHeight, PAGE_HEIGHT - 45));

    doc.setFont('NotoSans', 'bold');
    doc.setFontSize(8);
    doc.setTextColor(message.role === 'user' ? 30 : 40, message.role === 'user' ? 120 : 90, message.role === 'user' ? 105 : 150);
    doc.text(`${role}${date ? ` - ${date}` : ''}`, MARGIN, y);
    y += 5;
    doc.setFont('NotoSans', 'normal');
    doc.setFontSize(10);
    doc.setTextColor(25, 31, 43);

    for (const line of lines.length ? lines : ['']) {
      if (y > PAGE_HEIGHT - 20) addPage();
      doc.text(line, MARGIN + 4, y);
      y += 5;
    }
    y += 7;
  }

  const pages = doc.getNumberOfPages();
  for (let page = 1; page <= pages; page += 1) {
    doc.setPage(page);
    doc.setDrawColor(218, 224, 233);
    doc.line(MARGIN, PAGE_HEIGHT - 14, PAGE_WIDTH - MARGIN, PAGE_HEIGHT - 14);
    doc.setFont('NotoSans', 'normal');
    doc.setFontSize(8);
    doc.setTextColor(105, 114, 130);
    doc.text('Esportazione locale AURA', MARGIN, PAGE_HEIGHT - 9);
    doc.text(`${page} / ${pages}`, PAGE_WIDTH - MARGIN, PAGE_HEIGHT - 9, { align: 'right' });
  }
  return new Uint8Array(doc.output('arraybuffer'));
}

export async function saveConversationPdf(input: ConversationPdfInput): Promise<string> {
  const bytes = await createConversationPdf(input);
  const fileName = safeFileName(input.conversationTitle);
  if (isTauri()) {
    return invoke<string>('storage_export_pdf', { fileName, bytes: Array.from(bytes) });
  }
  const browserBuffer = new Uint8Array(bytes.byteLength);
  browserBuffer.set(bytes);
  const blob = new Blob([browserBuffer.buffer], { type: 'application/pdf' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  return fileName;
}
