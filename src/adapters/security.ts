import { invoke, isTauri } from '@tauri-apps/api/core';
import { evaluate } from '../core/policyEngine';
import type { ActionPlan, PolicyResult } from '../core/types';

export interface AuditStatus {
  valid: boolean;
  count: number;
  lastHash: string;
}

export async function evaluateAction(action: ActionPlan): Promise<PolicyResult> {
  if (!isTauri()) return evaluate(action);
  return invoke<PolicyResult>('policy_evaluate', {
    action: {
      id: action.id,
      capability: action.capability,
      summary: action.summary,
      scope: action.scope,
      reversible: action.reversible
    }
  });
}

export async function verifyAudit(): Promise<AuditStatus | undefined> {
  if (!isTauri()) return undefined;
  return invoke<AuditStatus>('audit_verify');
}
