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
  bubblewrap: boolean;
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

export interface EvolutionReportView {
  id: string;
  projectName: string;
  objective: string;
  state: string;
  promotion: string;
  createdAt: string;
  strongIsolationAvailable: boolean;
  executionAllowed: boolean;
  artifactPath: string;
  runId: string;
}

export interface EvolutionOperation {
  id: string;
  status: string;
  artifactPath: string;
}

export interface GuardrailStatus {
  installed: boolean;
  mode: string;
  blocking: boolean;
  reason: string;
}

export interface Creation3dStatus { installed: boolean; triposr: boolean; blender: boolean; outputRoot: string; reason: string }
export interface Creation3dResult { outputPath: string; device: string; format: string }
export interface BrowserStatus { implemented: boolean; interactive: boolean; userTakeover: boolean; networkPolicy: string }
export interface BrowserPage { url: string; title: string; text: string; truncated: boolean }
export interface ApiConnector { id: string; name: string; baseUrl: string; authKind: string; authHeader: string; hasSecret: boolean }
export interface ApiConnectorInput { id?: string; name: string; baseUrl: string; authKind: 'none' | 'bearer' | 'header'; authHeader: string; secret: string }
export interface ApiCallResult { status: number; contentType: string; body: string; truncated: boolean }
export interface RagDocument { id: string; sourceName: string; chunkCount: number; createdAt: string }
export interface RagHit { documentId: string; sourceName: string; ordinal: number; score: number; excerpt: string; citation: string }
export interface SecurityCheck { id: string; label: string; status: 'healthy'|'warning'|'unavailable'; detail: string }
export interface SecurityCenterStatus { moduleComplete: boolean; postureScore: number; checks: SecurityCheck[]; executableSha256: string; publicSignature: boolean; localSignature: boolean }
export interface RemoteStatus { paired: boolean; enabled: boolean; deviceName: string; relayHost: string; transport: string; inboundPort: boolean; allowedCommands: string[] }
export interface PairingResult { paired: boolean; pairingSecret: string; deviceName: string; relayHost: string; warning: string }

export type AllowedApplication =
  | 'calculator' | 'notepad' | 'explorer' | 'task-manager' | 'paint' | 'snipping-tool'
  | 'settings' | 'bluetooth-settings' | 'sound-settings' | 'security-settings';

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

export async function openApplication(application: AllowedApplication): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_open_application', { application });
}

export async function terminateProcess(pid: number): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_terminate_process', { pid });
}

export async function lockWorkstation(): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_lock_workstation');
}

export async function loadProfile(): Promise<PersonalProfile | null> {
  if (!isTauri()) return null;
  return invoke<PersonalProfile | null>('profile_load');
}

export async function saveProfile(profile: PersonalProfile): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('profile_save', { profile });
}

export async function clearProfile(): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('profile_clear');
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
    bubblewrap: false,
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

export async function listEvolutionProposals(): Promise<EvolutionReportView[]> {
  if (!isTauri()) return [];
  return invoke<EvolutionReportView[]>('evolution_list_proposals');
}

export async function generateEvolutionPatch(proposalId: string, projectPath: string): Promise<EvolutionOperation> {
  requireDesktop();
  return invoke<EvolutionOperation>('evolution_generate_patch', { proposalId, projectPath });
}

export async function runEvolutionSandbox(proposalId: string, projectPath: string, testProfile: 'Static' | 'Node' | 'Rust'): Promise<EvolutionOperation> {
  requireDesktop();
  return invoke<EvolutionOperation>('evolution_run_sandbox', { proposalId, projectPath, testProfile });
}

export async function promoteEvolutionPatch(proposalId: string, projectPath: string, validatedPatch: string): Promise<EvolutionOperation> {
  requireDesktop();
  return invoke<EvolutionOperation>('evolution_promote_patch', { proposalId, projectPath, validatedPatch });
}

export async function getGuardrailStatus(): Promise<GuardrailStatus> {
  if (!isTauri()) return {
    installed: false,
    mode: 'advisory',
    blocking: false,
    reason: 'Anteprima web: stato Guardian non disponibile.'
  };
  return invoke<GuardrailStatus>('guardrail_status');
}

export async function getCreation3dStatus(): Promise<Creation3dStatus> { requireDesktop(); return invoke('creation3d_status'); }
export async function generate3d(inputImage: string, jobName: string, device: 'Auto'|'Gpu'|'Cpu', meshResolution: number, format: 'glb'|'obj', bakeTexture: boolean): Promise<Creation3dResult> { requireDesktop(); return invoke('creation3d_generate', { inputImage, jobName, device, meshResolution, format, bakeTexture }); }
export async function open3dInBlender(meshPath: string): Promise<boolean> { requireDesktop(); return invoke('creation3d_open_blender', { meshPath }); }

export async function getBrowserStatus(): Promise<BrowserStatus> { requireDesktop(); return invoke('browser_status'); }
export async function openControlledBrowser(url: string): Promise<boolean> { requireDesktop(); return invoke('browser_open', { url }); }
export async function navigateControlledBrowser(url: string): Promise<boolean> { requireDesktop(); return invoke('browser_navigate', { url }); }
export async function fetchBrowserPage(url: string): Promise<BrowserPage> { requireDesktop(); return invoke('browser_fetch', { url }); }
export async function closeControlledBrowser(): Promise<boolean> { requireDesktop(); return invoke('browser_close'); }

export async function listApiConnectors(): Promise<ApiConnector[]> { requireDesktop(); return invoke('api_list_connectors'); }
export async function saveApiConnector(input: ApiConnectorInput): Promise<string> { requireDesktop(); return invoke('api_save_connector', { input }); }
export async function deleteApiConnector(id: string): Promise<boolean> { requireDesktop(); return invoke('api_delete_connector', { id }); }
export async function callApi(connectorId: string, method: string, path: string, bodyJson: string): Promise<ApiCallResult> { requireDesktop(); return invoke('api_call', { input: { connectorId, method, path, bodyJson } }); }

export async function listRagDocuments(): Promise<RagDocument[]> { requireDesktop(); return invoke('rag_list_documents'); }
export async function importRagDocument(path: string): Promise<{id:string;sourceName:string;chunkCount:number;model:string}> { requireDesktop(); return invoke('rag_import_document', { path }); }
export async function searchRag(query: string, limit = 5): Promise<RagHit[]> { requireDesktop(); return invoke('rag_search', { query, limit }); }
export async function deleteRagDocument(id: string): Promise<boolean> { requireDesktop(); return invoke('rag_delete_document', { id }); }
export async function getSecurityCenterStatus(): Promise<SecurityCenterStatus> { requireDesktop(); return invoke('security_center_status'); }
export async function runDefenderQuickScan(): Promise<boolean> { requireDesktop(); return invoke('security_quick_scan'); }
export async function updateDefenderDefinitions(): Promise<boolean> { requireDesktop(); return invoke('security_update_definitions'); }
export async function getRemoteStatus(): Promise<RemoteStatus> { requireDesktop(); return invoke('remote_status'); }
export async function pairRemote(deviceName: string, relayUrl: string): Promise<PairingResult> { requireDesktop(); return invoke('remote_pair', { deviceName, relayUrl }); }
export async function unpairRemote(): Promise<boolean> { requireDesktop(); return invoke('remote_unpair'); }
