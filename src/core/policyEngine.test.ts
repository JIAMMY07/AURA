import { describe, expect, it } from 'vitest';
import { evaluate } from './policyEngine';
import type { ActionPlan } from './types';
const base: ActionPlan={id:'1',capability:'system.inspect',summary:'Inspect',risk:'safe',scope:'local',reversible:true};
describe('policy engine',()=>{
  it('allows safe reads',()=>expect(evaluate(base).decision).toBe('allow'));
  it('requires sandbox for critical actions',()=>expect(evaluate({...base,risk:'critical'}).decision).toBe('sandbox'));
  it('denies credential export',()=>expect(evaluate({...base,capability:'credential.export'}).decision).toBe('deny'));
});
