import type { TurboModule } from 'react-native';
import { TurboModuleRegistry } from 'react-native';

// Keep Codegen declarations self-contained. React Native Codegen resolves
// TypeScript object declarations from this spec file; app-facing aliases live
// separately in MmpClient.ts and must not be imported here.
type NodeStatus = {
  tier: string;
  persistentStore: boolean;
  diagnosticsEnabled: boolean;
  lastTickMs: number;
};

type NodeMetrics = {
  storeLen: number;
  closeActionsDropped: number;
  eventQueueDropped: number;
  diagnosticRecords: number;
  diagnosticClamps: number;
  diagnosticDrops: number;
};

type Capabilities = {
  capabilities: string[];
  persistentStore: boolean;
};

type BetaSnapshot = {
  schemaVersion: number;
  status: NodeStatus;
  metrics: NodeMetrics;
  capabilities: Capabilities;
};

type FileSend = {
  messageId: string;
  root: string;
};

type FileProgress = {
  state: string;
  haveSymbols: number;
  totalSymbols: number;
  stripesDone: number;
  stripes: number;
};

type OutFrame = {
  link: number;
  /** Base64-encoded encrypted/framed bytes; binary arrays never cross JS. */
  bytes: string;
};

type NativeFileOffer = {
  nameB64: string;
  mimeB64: string;
  size: number;
  root: string;
  thumbRoot: string | null;
};

/**
 * Tagged native event record returned by bounded polling. Optional fields keep
 * the Codegen declaration self-contained while preserving every known FfiEvent
 * payload field for MmpClient's explicit fail-closed normalization.
 */
type NativePollEvent = {
  type: string;
  conv?: string;
  from?: string;
  id?: string;
  body?: string;
  apptype?: number;
  parents?: string[];
  state?: string;
  owner?: string;
  field?: number;
  peer?: string;
  since?: number;
  successor?: string | null;
  gid?: string;
  epoch?: number;
  tier?: string;
  scope?: string;
  annType?: number;
  notAfter?: number;
  text?: string;
  official?: boolean;
  effectivePriority?: number;
  scopeNameHint?: string | null;
  sharer?: string;
  entries?: NativeFileOffer[];
  root?: string;
  path?: string;
  reason?: string;
};

/** TurboModule codegen surface; native implementations validate string enums. */
export interface Spec extends TurboModule {
  start(tier: string): Promise<string>;
  stop(): Promise<void>;
  setTier(tier: string): Promise<void>;
  status(): Promise<NodeStatus>;
  metrics(): Promise<NodeMetrics>;
  capabilities(): Promise<Capabilities>;
  betaSnapshot(): Promise<BetaSnapshot>;
  setWallClock(unixS: number): Promise<void>;
  pinAuthorityRoot(scope: string, issuerPublicKeyB64: string): Promise<void>;
  subscribeScope(scopeName: string): Promise<string>;
  unsubscribeScope(scope: string): Promise<void>;
  mintAuthCert(
    suite: number,
    subjectPublicKeyB64: string,
    scope: string,
    caps: number,
    priorityBudget: number,
    notAfter: number,
  ): Promise<string>;
  publishAnnouncement(
    scope: string,
    annType: number,
    priority: number,
    text: string,
    certB64: string | null,
  ): Promise<string>;
  nodeId(): Promise<string>;
  publicKey(): Promise<string>;
  sendText(peer: string, text: string): Promise<string>;
  /** Returns the Rust enum spelling; MmpClient performs explicit mapping. */
  messageState(id: string): Promise<string | null>;
  sendBody(peer: string, apptype: number, payloadB64: string): Promise<string>;
  sendFile(peer: string, name: string, mime: string, dataB64: string): Promise<FileSend>;
  fetchFile(root: string, dest: string): Promise<void>;
  cancelFetch(root: string): Promise<void>;
  fileProgress(root: string): Promise<FileProgress | null>;
  pollSendBatch(maxItems: number): Promise<OutFrame[]>;
  pollEventBatch(maxItems: number): Promise<NativePollEvent[]>;
  pollCloseBatch(maxItems: number): Promise<number[]>;
  createGroup(): Promise<string>;
  groupInvite(gid: string, peer: string): Promise<void>;
  groupSend(gid: string, text: string): Promise<string>;
  groupSendBody(gid: string, apptype: number, payloadB64: string): Promise<string>;
  groupMembers(gid: string): Promise<string[]>;
  groupRemove(gid: string, member: string): Promise<number>;
  setNickname(name: string, seq: number): Promise<void>;
  nicknameOf(peer: string): Promise<string | null>;
  revokeIdentity(reason: number): Promise<string>;
  tryRevokeIdentity(reason: number): Promise<string>;
  revokeIdentityWithSuccessor(reason: number, successorPublicKeyB64: string, countersigB64: string): Promise<string>;
  ownClaims(): Promise<string[]>;
  ingestClaim(claimB64: string, verified: boolean): Promise<void>;
  block(peer: string): Promise<void>;
  requestFriendship(peerPublicKeyB64: string): Promise<void>;
  allowFriendshipFrom(peerPublicKeyB64: string): Promise<void>;
  enableDiagnostics(): Promise<void>;
  disableDiagnostics(): Promise<void>;
  diagDump(): Promise<string>;
  storeLen(): Promise<number>;
  flush(): Promise<void>;
  lastPersistError(): Promise<string | null>;
  addListener(eventName: string): void;
  removeListeners(count: number): void;
}

export default TurboModuleRegistry.getEnforcing<Spec>('MmpMesh');
