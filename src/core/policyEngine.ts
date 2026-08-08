import type { ActionPlan, PolicyResult } from './types';

const blocked = new Set(['credential.export', 'security.disable', 'privilege.escalate']);

export function evaluate(plan: ActionPlan): PolicyResult {
  if (blocked.has(plan.capability)) return { decision: 'deny', risk: 'critical', reason: 'Capability vietata dalla policy di base.' };
  if (plan.risk === 'critical') return { decision: 'sandbox', risk: 'critical', reason: 'Test in sandbox/VM e doppia conferma richiesti.' };
  if (plan.risk === 'sensitive') return { decision: 'confirm', risk: 'sensitive', reason: 'Conferma esplicita richiesta prima dell’esecuzione.' };
  if (plan.risk === 'moderate') return { decision: 'confirm', risk: 'moderate', reason: 'Conferma richiesta finché lo scope non viene autorizzato.' };
  return { decision: 'allow', risk: 'safe', reason: 'Azione di sola lettura o a basso rischio nello scope consentito.' };
}
