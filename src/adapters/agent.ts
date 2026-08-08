import { invoke, isTauri } from '@tauri-apps/api/core';

export interface DiskSnapshot {
  name: string;
  mountPoint: string;
  availableBytes: number;
  totalBytes: number;
}

export interface SystemSnapshot {
  osName: string;
  osVersion: string;
  cpuUsagePercent: number;
  logicalCpus: number;
  memoryUsedBytes: number;
  memoryTotalBytes: number;
  processCount: number;
  uptimeSeconds: number;
  disks: DiskSnapshot[];
}

export interface ProcessSnapshot {
  pid: number;
  name: string;
  memoryBytes: number;
  cpuUsagePercent: number;
}

export interface PersonalProfile {
  displayName: string;
  communicationStyle: string;
  goals: string;
  preferences: string;
  constraints: string;
  projectContext: string;
}

export interface SandboxStatus {
  strongIsolationAvailable: boolean;
  windowsSandbox: boolean;
  wsl: boolean;
  containerRuntime: boolean;
  simulationAvailable: boolean;
  executionAllowed: boolean;
  reason: string;
}

export interface EvolutionProposal {
  id: string;
  reportPath: string;
  executable: boolean;
  status: string;
}

function requireDesktop() {
  if (!isTauri()) throw new Error('Funzione disponibile soltanto nell’app desktop AURA.');
}

export async function getSystemSnapshot(): Promise<SystemSnapshot> {
  requireDesktop();
  return invoke<SystemSnapshot>('pc_system_snapshot');
}

export async function listProcesses(): Promise<ProcessSnapshot[]> {
  requireDesktop();
  return invoke<ProcessSnapshot[]>('pc_list_processes');
}

export async function openApplication(application: 'calculator' | 'notepad' | 'explorer'): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_open_application', { application });
}

export async function loadProfile(): Promise<PersonalProfile | null> {
  if (!isTauri()) return null;
  return invoke<PersonalProfile | null>('profile_load');
}

export async function saveProfile(profile: PersonalProfile): Promise<void> {
  requireDesktop();
  return invoke('profile_save', { profile });
}

export async function clearProfile(): Promise<void> {
  requireDesktop();
  return invoke('profile_clear');
}

export async function recordFeedback(messageId: string, rating: 'helpful' | 'correction', correction = ''): Promise<void> {
  requireDesktop();
  return invoke('feedback_record', { feedback: { messageId, rating, correction } });
}

export async function getSandboxStatus(): Promise<SandboxStatus> {
  if (!isTauri()) return {
    strongIsolationAvailable: false,
    windowsSandbox: false,
    wsl: false,
    containerRuntime: false,
    simulationAvailable: true,
    executionAllowed: false,
    reason: 'Anteprima web: esecuzione sandbox disabilitata.'
  };
  return invoke<SandboxStatus>('sandbox_status');
}

export async function recordEvolutionProposal(projectName: string, objective: string): Promise<EvolutionProposal> {
  requireDesktop();
  return invoke<EvolutionProposal>('evolution_record_proposal', { projectName, objective });
}
