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

/** TurboModule codegen surface; native implementations validate string enums. */
export interface Spec extends TurboModule {
  start(tier: string): Promise<string>;
  stop(): Promise<void>;
  setTier(tier: string): Promise<void>;
  status(): Promise<NodeStatus>;
  metrics(): Promise<NodeMetrics>;
  nodeId(): Promise<string>;
  publicKey(): Promise<string>;
  sendText(peer: string, text: string): Promise<string>;
  /** Returns the Rust enum spelling; MmpClient performs explicit mapping. */
  messageState(id: string): Promise<string | null>;
  sendFile(peer: string, name: string, mime: string, dataB64: string): Promise<FileSend>;
  fetchFile(root: string, dest: string): Promise<void>;
  cancelFetch(root: string): Promise<void>;
  fileProgress(root: string): Promise<FileProgress | null>;
  createGroup(): Promise<string>;
  groupInvite(gid: string, peer: string): Promise<void>;
  groupSend(gid: string, text: string): Promise<string>;
  groupMembers(gid: string): Promise<string[]>;
  groupRemove(gid: string, member: string): Promise<number>;
  setNickname(name: string, seq: number): Promise<void>;
  revokeIdentity(reason: number): Promise<string>;
  tryRevokeIdentity(reason: number): Promise<string>;
  revokeIdentityWithSuccessor(reason: number, successorPublicKeyB64: string, countersigB64: string): Promise<string>;
  ownClaims(): Promise<string[]>;
  ingestClaim(claimB64: string, verified: boolean): Promise<void>;
  block(peer: string): Promise<void>;
  enableDiagnostics(): Promise<void>;
  diagDump(): Promise<string>;
  storeLen(): Promise<number>;
  flush(): Promise<void>;
  lastPersistError(): Promise<string | null>;
  addListener(eventName: string): void;
  removeListeners(count: number): void;
}

export default TurboModuleRegistry.getEnforcing<Spec>('MmpMesh');
