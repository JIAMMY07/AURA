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
  canaryAvailable: boolean;
  automaticRollback: boolean;
  attestationVersion: number;
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
  inspection?: EvolutionInspection;
  diffApproved: boolean;
  attempts: number;
  attestation?: EvolutionAttestation;
  rollbackStatus: string;
}

export interface EvolutionAttestation {
  schemaVersion: number; runId: string; testProfile: string; patchSha256: string;
  dependencyPolicy: string; networkDuringTest: boolean; hostFilesystemMountedReadOnly: boolean;
  canaryPassed: boolean; canaryFreshNamespace: boolean; canaryNetworkDisabled: boolean;
  changedFiles: string[]; securityFindings: string[]; createdAtUtc: string;
}

export interface WorkspaceGrant { id: string; name: string; path: string; createdAt: string }
export interface WorkspaceEntry { relativePath: string; name: string; isDirectory: boolean; sizeBytes: number; modifiedUnix: number }
export interface WorkspaceReadResult { relativePath: string; content: string; sizeBytes: number }
export interface InstalledApplication { id: string; label: string; source: string }
export interface MemoryStatus { snapshots: number; lessons: number; lastConsolidatedAt: string; automatic: boolean }

export interface EvolutionInspection {
  changedFiles: string[];
  capabilities: string[];
  dependencyChanges: string[];
  securityFindings: string[];
  risk: 'safe' | 'moderate' | 'critical';
  patchSha256: string;
  diffExcerpt: string;
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
export interface BrowserStatus { implemented: boolean; interactive: boolean; userTakeover: boolean; automation: boolean; allowedActions: string[]; networkPolicy: string; currentUrl?: string }
export interface BrowserPage { url: string; title: string; text: string; truncated: boolean }
export interface ScreenAnalysis { description: string; model: string; width: number; height: number; stored: boolean }
export interface TaskStopResult {
  generationsCancelled: number;
  childProcessesSignalled: number;
  ttsStopped: boolean;
  browserClosed: boolean;
  message: string;
}
export interface AiProviderDescriptor {
  id: string;
  displayName: string;
  apiKind: string;
  endpoint?: string;
  configured: boolean;
  active: boolean;
  reachable: boolean;
  localOnly: boolean;
  contractVersion: number;
  capabilities: string[];
  notes: string;
}
export interface WebSearchResult { title: string; url: string; snippet: string }
export interface WebSearchResponse { query: string; provider: string; searchedAtUnix: number; results: WebSearchResult[] }
export interface BrowserElement { selector: string; kind: string; label: string; disabled: boolean; value?: string }
export interface BrowserSnapshot extends BrowserPage { elements: BrowserElement[] }
export type BrowserActionName = 'click'|'type'|'submit'|'scroll';
export type BrowserControlName = 'back'|'forward'|'reload';
export interface BrowserAgentStatus { running: boolean; phase: string; steps: number; maxSteps: number; currentUrl?: string; lastAction?: string; message: string; pageRevision: number; profileIsolated: boolean; pauseRequested: boolean }
export interface QuarantinedDownload { id: string; originalName: string; storedPath: string; bytes: number; contentType: string; sha256: string; executable: boolean; released: boolean }
export interface ApiConnector { id: string; name: string; baseUrl: string; authKind: string; authHeader: string; hasSecret: boolean }
export interface ApiConnectorInput { id?: string; name: string; baseUrl: string; authKind: 'none' | 'bearer' | 'header' | 'basic' | 'query'; authHeader: string; secret: string }
export interface ApiCallResult { status: number; contentType: string; body: string; truncated: boolean }
export interface RagDocument { id: string; sourceName: string; chunkCount: number; createdAt: string }
export interface RagHit { documentId: string; sourceName: string; ordinal: number; score: number; excerpt: string; citation: string }
export interface SecurityCheck { id: string; label: string; status: 'healthy'|'warning'|'unavailable'; detail: string }
export interface SecurityCenterStatus { moduleComplete: boolean; coverageScore: number; postureScore: number; checks: SecurityCheck[]; executableSha256: string; publicSignature: boolean; localSignature: boolean; signatureStatus: string; signer: string; timestamped: boolean; recommendations: string[] }
export interface IncidentStatus { severity: 'healthy'|'warning'|'high'|'critical'; activeThreats: number; recentDetections: number; establishedConnections: number; listeningEndpoints: number; defenderHealthy: boolean; firewallHealthy: boolean; auraIntegrity: 'verified'|'unverified'|'compromised'; automaticContainment: boolean; summary: string; recommendations: string[] }
export interface RemediationResult { confirmed: boolean; threatsBefore: number; threatsAfter: number; scanCompleted: boolean; contained: boolean; message: string }
export interface PrivacyStatus { databaseIntegrity: boolean; secureDelete: boolean; foreignKeys: boolean; walEnabled: boolean; encryptedPayloads: boolean; plaintextPayloadRows: number; auditChainValid: boolean; auditedEvents: number; keyStore: string; metadataNotice: string }
export type PcAccessMode = 'read-only' | 'controlled' | 'sandbox-auto';
export interface PcAccessStatus { mode: PcAccessMode; label: string; sandboxScope: string; network: string; approvals: string; warning: string }
export interface RemoteStatus { paired: boolean; enabled: boolean; deviceName: string; relayHost: string; transport: string; inboundPort: boolean; allowedCommands: string[] }
export interface PairingResult { paired: boolean; pairingSecret: string; deviceName: string; relayHost: string; warning: string }
export interface RemotePollResult { received: boolean; command?: { id:string; command:string; payloadJson:string; policy:string; executable:boolean; reason:string }; outcome:string }

export type AllowedApplication =
  | 'calculator' | 'notepad' | 'explorer' | 'task-manager' | 'paint' | 'snipping-tool'
  | 'settings' | 'bluetooth-settings' | 'sound-settings' | 'display-settings' | 'notification-settings'
  | 'microphone-privacy' | 'windows-update' | 'downloads' | 'documents' | 'security-settings';

export type MediaAction = 'volume-up' | 'volume-down' | 'mute' | 'play-pause' | 'next' | 'previous';
export type HandPointerAction = 'left-click' | 'double-click' | 'left-down' | 'left-up' | 'right-click';
export type HandWindowAction = 'minimize' | 'maximize-restore' | 'switch-next' | 'close';
export type WindowControlAction = 'minimize' | 'maximize-restore' | 'close';

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

export async function getPcAccessMode(): Promise<PcAccessStatus> {
  if (!isTauri()) return { mode: 'controlled', label: 'Controllato', sandboxScope: 'Anteprima web', network: 'Strumenti desktop non disponibili', approvals: 'Conferme simulate', warning: 'Apri AURA desktop per applicare il profilo.' };
  return invoke<PcAccessStatus>('policy_get_access_mode');
}

export async function setPcAccessMode(mode: PcAccessMode): Promise<PcAccessStatus> {
  if (!isTauri()) return { mode, label: mode === 'read-only' ? 'Sola lettura' : mode === 'sandbox-auto' ? 'Autonomia in sandbox' : 'Controllato', sandboxScope: 'Anteprima web', network: 'Strumenti desktop non disponibili', approvals: 'Conferme simulate', warning: 'Apri AURA desktop per applicare il profilo.' };
  return invoke<PcAccessStatus>('policy_set_access_mode', { mode });
}

export async function listInstalledApplications(): Promise<InstalledApplication[]> {
  if (!isTauri()) return [];
  return invoke<InstalledApplication[]>('pc_list_installed_applications');
}

export async function openInstalledApplication(applicationId: string): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_open_installed_application', { applicationId });
}
export interface SandboxInstallResult { started: boolean; restartMayBeRequired: boolean; message: string }

export async function getPrivacyStatus(): Promise<PrivacyStatus> {
  requireDesktop();
  return invoke<PrivacyStatus>('privacy_status');
}

export async function controlMedia(action: MediaAction): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_media_control', { action });
}

export async function setHandDesktopControl(enabled: boolean): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_hand_control_set', { enabled });
}

export async function moveHandPointer(x: number, y: number): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_hand_pointer_move', { input: { x, y } });
}

export async function performHandPointerAction(action: HandPointerAction): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_hand_pointer_action', { action });
}

export async function performHandScroll(delta: number): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_hand_scroll', { input: { delta } });
}

export async function performHandWindowAction(action: HandWindowAction): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_hand_window_action', { action });
}

export async function analyzeScreen(prompt = ''): Promise<ScreenAnalysis> {
  requireDesktop();
  return invoke<ScreenAnalysis>('screen_analyze', { prompt });
}

export async function stopAllTasks(): Promise<TaskStopResult> {
  requireDesktop();
  return invoke<TaskStopResult>('task_stop_all');
}

export async function listAiProviders(): Promise<AiProviderDescriptor[]> {
  if (!isTauri()) return [];
  return invoke<AiProviderDescriptor[]>('ai_provider_list');
}

export async function controlActiveWindow(action: WindowControlAction): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('pc_window_control', { action });
}

export async function listWorkspaceGrants(): Promise<WorkspaceGrant[]> {
  if (!isTauri()) return [];
  return invoke<WorkspaceGrant[]>('workspace_list_grants');
}

export async function addWorkspaceGrant(path: string): Promise<WorkspaceGrant | null> {
  requireDesktop();
  return invoke<WorkspaceGrant | null>('workspace_add_grant', { path });
}

export async function removeWorkspaceGrant(grantId: string): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('workspace_remove_grant', { grantId });
}

export async function listWorkspaceEntries(grantId: string, relativePath = ''): Promise<WorkspaceEntry[]> {
  requireDesktop();
  return invoke<WorkspaceEntry[]>('workspace_list_entries', { grantId, relativePath });
}

export async function searchWorkspace(grantId: string, query: string): Promise<WorkspaceEntry[]> {
  requireDesktop();
  return invoke<WorkspaceEntry[]>('workspace_search', { grantId, query });
}

export async function readWorkspaceText(grantId: string, relativePath: string): Promise<WorkspaceReadResult> {
  requireDesktop();
  return invoke<WorkspaceReadResult>('workspace_read_text', { grantId, relativePath });
}

export async function writeWorkspaceText(grantId: string, relativePath: string, content: string): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('workspace_write_text', { grantId, relativePath, content });
}

export async function createWorkspaceDirectory(grantId: string, relativePath: string): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('workspace_create_directory', { grantId, relativePath });
}

export async function copyWorkspaceItem(grantId: string, source: string, destination: string): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('workspace_copy', { grantId, source, destination });
}

export async function moveWorkspaceItem(grantId: string, source: string, destination: string): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('workspace_move', { grantId, source, destination });
}

export async function trashWorkspaceItem(grantId: string, relativePath: string): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('workspace_trash', { grantId, relativePath });
}

export async function openWorkspaceItem(grantId: string, relativePath: string): Promise<boolean> {
  requireDesktop();
  return invoke<boolean>('workspace_open', { grantId, relativePath });
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
    canaryAvailable: false,
    automaticRollback: true,
    attestationVersion: 2,
    reason: 'Anteprima web: esecuzione sandbox disabilitata.'
  };
  return invoke<SandboxStatus>('sandbox_status');
}

export async function getMemoryStatus(): Promise<MemoryStatus> {
  requireDesktop(); return invoke<MemoryStatus>('memory_status');
}
export async function consolidateMemory(): Promise<MemoryStatus> {
  requireDesktop(); return invoke<MemoryStatus>('memory_consolidate');
}
export async function setAutomaticMemory(enabled: boolean): Promise<MemoryStatus> {
  requireDesktop(); return invoke<MemoryStatus>('memory_set_automatic', { enabled });
}
export async function rollbackMemory(): Promise<MemoryStatus> {
  requireDesktop(); return invoke<MemoryStatus>('memory_rollback');
}

export async function detectEvolutionProjectRoot(): Promise<string | null> {
  if (!isTauri()) return null;
  return invoke<string | null>('evolution_detect_project_root');
}

export async function installSandboxRuntime(): Promise<SandboxInstallResult> {
  requireDesktop();
  return invoke<SandboxInstallResult>('sandbox_install_runtime');
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

export async function inspectEvolutionPatch(proposalId: string): Promise<EvolutionInspection> {
  requireDesktop();
  return invoke<EvolutionInspection>('evolution_inspect_patch', { proposalId });
}

export async function approveEvolutionPatch(proposalId: string): Promise<EvolutionOperation> {
  requireDesktop();
  return invoke<EvolutionOperation>('evolution_approve_patch', { proposalId });
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
export async function searchInternet(query: string, limit = 6): Promise<WebSearchResponse> { requireDesktop(); return invoke('browser_search', { query, limit }); }
export async function snapshotControlledBrowser(): Promise<BrowserSnapshot> { requireDesktop(); return invoke('browser_snapshot'); }
export async function closeControlledBrowser(): Promise<boolean> { requireDesktop(); return invoke('browser_close'); }
export async function actInControlledBrowser(action: BrowserActionName, selector: string, value = ''): Promise<boolean> { requireDesktop(); return invoke('browser_act', { input: { action, selector, value } }); }
export async function controlBrowser(action: BrowserControlName): Promise<boolean> { requireDesktop(); return invoke('browser_control', { input: { action } }); }
export async function getBrowserAgentStatus(): Promise<BrowserAgentStatus> { requireDesktop(); return invoke('browser_agent_status'); }
export async function startBrowserAgent(goal: string, maxSteps: number): Promise<BrowserAgentStatus> { requireDesktop(); return invoke('browser_agent_start', { input: { goal, maxSteps } }); }
export async function pauseBrowserAgent(): Promise<BrowserAgentStatus> { requireDesktop(); return invoke('browser_agent_pause'); }
export async function quarantineBrowserDownload(url: string): Promise<QuarantinedDownload> { requireDesktop(); return invoke('browser_quarantine_download', { url }); }

export async function listApiConnectors(): Promise<ApiConnector[]> { requireDesktop(); return invoke('api_list_connectors'); }
export async function saveApiConnector(input: ApiConnectorInput): Promise<string> { requireDesktop(); return invoke('api_save_connector', { input }); }
export async function deleteApiConnector(id: string): Promise<boolean> { requireDesktop(); return invoke('api_delete_connector', { id }); }
export async function callApi(connectorId: string, method: string, path: string, bodyJson: string): Promise<ApiCallResult> { requireDesktop(); return invoke('api_call', { input: { connectorId, method, path, bodyJson } }); }

export async function listRagDocuments(): Promise<RagDocument[]> { requireDesktop(); return invoke('rag_list_documents'); }
export async function importRagDocument(path: string): Promise<{id:string;sourceName:string;chunkCount:number;model:string}> { requireDesktop(); return invoke('rag_import_document', { path }); }
export async function searchRag(query: string, limit = 5): Promise<RagHit[]> { requireDesktop(); return invoke('rag_search', { query, limit }); }
export async function deleteRagDocument(id: string): Promise<boolean> { requireDesktop(); return invoke('rag_delete_document', { id }); }
export async function getSecurityCenterStatus(): Promise<SecurityCenterStatus> { requireDesktop(); return invoke('security_center_status'); }
export async function getIncidentStatus(): Promise<IncidentStatus> { requireDesktop(); return invoke('security_incident_status'); }
export async function remediateDefenderThreats(): Promise<RemediationResult> { requireDesktop(); return invoke('security_remediate_threats'); }
export async function runDefenderQuickScan(): Promise<boolean> { requireDesktop(); return invoke('security_quick_scan'); }
export async function updateDefenderDefinitions(): Promise<boolean> { requireDesktop(); return invoke('security_update_definitions'); }
export async function getRemoteStatus(): Promise<RemoteStatus> { requireDesktop(); return invoke('remote_status'); }
export async function pairRemote(deviceName: string, relayUrl: string): Promise<PairingResult> { requireDesktop(); return invoke('remote_pair', { deviceName, relayUrl }); }
export async function unpairRemote(): Promise<boolean> { requireDesktop(); return invoke('remote_unpair'); }
export async function pollRemoteOnce(): Promise<RemotePollResult> { requireDesktop(); return invoke('remote_poll_once'); }
