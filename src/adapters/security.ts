import { invoke, isTauri } from '@tauri-apps/api/core';
import { evaluate } from '../core/policyEngine';
import type { ActionPlan, PolicyResult } from '../core/types';

export interface AuditStatus {
  valid: boolean;
  count: number;
  lastHash: string;
}

export interface AuditEvent {
  sequence: number;
  correlationId: string;
  eventType: string;
  decision: string;
  summary: string;
  payload?: unknown;
  createdAt: string;
  eventHash: string;
}

export type InteractionSource = 'voice' | 'alexa' | 'gesture';
export interface InteractionLogInput {
  source: InteractionSource;
  action: string;
  content: string;
  outcome: 'allow' | 'confirm' | 'deny' | 'error' | 'caution';
  correlationId?: string;
  metadata?: Record<string, unknown>;
}

export async function evaluateAction(action: ActionPlan): Promise<PolicyResult> {
  if (!isTauri()) return evaluate(action);
  return invoke<PolicyResult>('policy_evaluate', {
    action: {
      id: action.id,
      capability: action.capability,
      summary: action.summary,
      scope: action.scope
    }
  });
}

export async function verifyAudit(): Promise<AuditStatus | undefined> {
  if (!isTauri()) return undefined;
  return invoke<AuditStatus>('audit_verify');
}

export async function listAuditEvents(options: {
  limit?: number;
  eventType?: string;
  decision?: string;
  includePayloads?: boolean;
} = {}): Promise<AuditEvent[]> {
  if (!isTauri()) return [];
  return invoke<AuditEvent[]>('audit_list', {
    limit: options.limit ?? 200,
    eventType: options.eventType,
    decision: options.decision,
    includePayloads: options.includePayloads ?? false
  });
}

export async function exportAuditLog(includePayloads = false): Promise<string> {
  if (!isTauri()) throw new Error('Esportazione disponibile nell’app desktop AURA.');
  return invoke<string>('audit_export', { includePayloads });
}

export async function recordInteraction(input: InteractionLogInput): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>('audit_record_interaction', { input });
}
