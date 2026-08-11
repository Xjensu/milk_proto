import type { TurboModule } from 'react-native';
import { TurboModuleRegistry } from 'react-native';

/** TurboModule codegen surface; native implementations validate string enums. */
export interface Spec extends TurboModule {
  start(tier: string): Promise<string>;
  stop(): Promise<void>;
  setTier(tier: string): Promise<void>;
  nodeId(): Promise<string>;
  publicKey(): Promise<string>;
  sendText(peer: string, text: string): Promise<string>;
  messageState(id: string): Promise<string | null>;
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
