export type Risk = 'safe' | 'moderate' | 'sensitive' | 'critical';
export type Decision = 'allow' | 'confirm' | 'sandbox' | 'deny';

export interface ActionPlan {
  id: string;
  capability: string;
  summary: string;
  risk: Risk;
  scope: string;
  reversible: boolean;
}

export interface PolicyResult {
  decision: Decision;
  risk: Risk;
  reason: string;
}
