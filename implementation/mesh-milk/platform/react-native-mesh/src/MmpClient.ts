import { NativeEventEmitter, NativeModules } from 'react-native';

// ---- wire types (mirror the FFI surface; keep in sync with mmp-ffi) --------

export type NodeIdHex = string;   // 32 lowercase hex chars (16 bytes)
export type MsgIdHex = string;    // 32 lowercase hex chars (16 bytes)
export type GroupIdHex = string;  // 32 lowercase hex chars (16 bytes)

export type Tier = 'leaf' | 'relay' | 'anchor';

export type DeliveryState = 'queued' | 'inMesh' | 'delivered' | 'expired';

export type MmpEvent =
  | { type: 'messageReceived'; conv: MsgIdHex; from: NodeIdHex; id: MsgIdHex; body: string; apptype: number }
  | { type: 'messageGap'; conv: MsgIdHex; parents: MsgIdHex[] }
  | { type: 'delivered'; id: MsgIdHex }
  | { type: 'deliveryProgress'; id: MsgIdHex; state: DeliveryState }
  | { type: 'claimUpdated'; owner: NodeIdHex; field: number }
  | { type: 'compromiseWarning'; owner: NodeIdHex }
  | { type: 'peerRevoked'; peer: NodeIdHex; successor: string | null }
  | { type: 'contactSeen'; peer: NodeIdHex }
  | { type: 'established'; peer: NodeIdHex }
  | { type: 'groupInvite'; gid: GroupIdHex; from: NodeIdHex }
  | { type: 'groupMessage'; gid: GroupIdHex; from: NodeIdHex; id: MsgIdHex; body: string }
  | { type: 'groupEpochChanged'; gid: GroupIdHex; epoch: number }
  | { type: 'tierChanged'; tier: Tier };

/** The native methods consumed by this package. Link bytes are intentionally absent. */
export interface MmpMeshNative {
  // Lifecycle. Idempotent: if a node already exists for this seed/store, returns its id.
  start(tier: Tier): Promise<NodeIdHex>;
  stop(): Promise<void>;
  setTier(tier: Tier): Promise<void>;

  // Identity / claims.
  nodeId(): Promise<NodeIdHex>;
  publicKey(): Promise<string>;
  ownClaims(): Promise<string[]>;
  ingestClaim(claimB64: string, verified: boolean): Promise<void>;
  setNickname(name: string, seq: number): Promise<void>;
  revokeIdentity(reason: number): Promise<string>;
  tryRevokeIdentity(reason: number): Promise<string>;
  revokeIdentityWithSuccessor(reason: number, successorPublicKeyB64: string, countersigB64: string): Promise<string>;
  block(peer: NodeIdHex): Promise<void>;

  // Messaging.
  sendText(peer: NodeIdHex, text: string): Promise<MsgIdHex>;
  messageState(id: MsgIdHex): Promise<DeliveryState | null>;

  // Groups.
  createGroup(): Promise<GroupIdHex>;
  groupInvite(gid: GroupIdHex, peer: NodeIdHex): Promise<void>;
  groupSend(gid: GroupIdHex, text: string): Promise<MsgIdHex>;
  groupMembers(gid: GroupIdHex): Promise<NodeIdHex[]>;
  groupRemove(gid: GroupIdHex, member: NodeIdHex): Promise<number>;

  // Diagnostics.
  enableDiagnostics(): Promise<void>;
  diagDump(): Promise<string>;

  // Storage / persistence.
  storeLen(): Promise<number>;
  flush(): Promise<void>;
  lastPersistError(): Promise<string | null>;

  // NativeEventEmitter requires these no-ops on iOS; Android ignores them.
  addListener(eventName: string): void;
  removeListeners(count: number): void;
}

export interface MmpEventSubscription {
  remove(): void;
}

export interface MmpEventEmitter {
  addListener(eventName: string, handler: (event: MmpEvent) => void): MmpEventSubscription;
}

const REQUIRED_NATIVE_METHODS = [
  'start',
  'stop',
  'setTier',
  'nodeId',
  'publicKey',
  'ownClaims',
  'ingestClaim',
  'setNickname',
  'revokeIdentity',
  'tryRevokeIdentity',
  'revokeIdentityWithSuccessor',
  'block',
  'sendText',
  'messageState',
  'createGroup',
  'groupInvite',
  'groupSend',
  'groupMembers',
  'groupRemove',
  'enableDiagnostics',
  'diagDump',
  'storeLen',
  'flush',
  'lastPersistError',
  'addListener',
  'removeListeners',
] as const;

export class MmpNativeModuleError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'MmpNativeModuleError';
  }
}

/** Validate the injected bridge and fail with an actionable error before any call crosses it. */
export function requireMmpNativeModule(value?: unknown): MmpMeshNative {
  const candidate = value === undefined ? NativeModules?.MmpMesh : value;
  if (candidate === null || typeof candidate !== 'object') {
    throw new MmpNativeModuleError(
      'MMP native module MmpMesh is unavailable. Build and register the Kotlin/Swift native module before creating MmpClient.',
    );
  }

  const missing = REQUIRED_NATIVE_METHODS.filter(
    (method) => typeof (candidate as Record<string, unknown>)[method] !== 'function',
  );
  if (missing.length > 0) {
    throw new MmpNativeModuleError(`MMP native module MmpMesh is incomplete; missing: ${missing.join(', ')}`);
  }

  return candidate as MmpMeshNative;
}

function defaultEventEmitter(native: MmpMeshNative): MmpEventEmitter {
  type NativeModuleConstructorArg = ConstructorParameters<typeof NativeEventEmitter>[0];
  return new NativeEventEmitter(native as unknown as NativeModuleConstructorArg) as unknown as MmpEventEmitter;
}

export class MmpClient {
  private readonly nativeSource: unknown;
  private readonly usesDefaultNative: boolean;
  private readonly injectedEmitter?: MmpEventEmitter;
  private resolvedNative: MmpMeshNative | null = null;
  private resolvedEmitter: MmpEventEmitter | null = null;

  /**
   * Inject the platform module for tests or a hand-written/TurboModule implementation.
   * The no-argument singleton is lazy so importing this package before native registration
   * is safe; its first operation still fails with MmpNativeModuleError when the module is absent.
   */
  constructor(nativeModule?: unknown, eventEmitter?: MmpEventEmitter) {
    this.usesDefaultNative = arguments.length === 0 || nativeModule === undefined;
    this.nativeSource = nativeModule;
    this.injectedEmitter = eventEmitter;
    if (!this.usesDefaultNative) {
      this.resolvedNative = requireMmpNativeModule(nativeModule);
      this.resolvedEmitter = eventEmitter ?? defaultEventEmitter(this.resolvedNative);
    }
  }

  private get native(): MmpMeshNative {
    if (this.resolvedNative === null) {
      this.resolvedNative = requireMmpNativeModule(this.usesDefaultNative ? undefined : this.nativeSource);
    }
    return this.resolvedNative;
  }

  private get emitter(): MmpEventEmitter {
    if (this.resolvedEmitter === null) {
      this.resolvedEmitter = this.injectedEmitter ?? defaultEventEmitter(this.native);
    }
    return this.resolvedEmitter;
  }

  /** Starts (or attaches to) the native mesh service. Call once, e.g. from App root. */
  async start(tier: Tier = 'leaf'): Promise<NodeIdHex> {
    return this.native.start(tier);
  }

  async stop(): Promise<void> {
    return this.native.stop();
  }

  setTier(tier: Tier): Promise<void> {
    return this.native.setTier(tier);
  }

  nodeId(): Promise<NodeIdHex> {
    return this.native.nodeId();
  }

  publicKey(): Promise<string> {
    return this.native.publicKey();
  }

  sendText(peer: NodeIdHex, text: string): Promise<MsgIdHex> {
    return this.native.sendText(peer, text);
  }

  messageState(id: MsgIdHex): Promise<DeliveryState | null> {
    return this.native.messageState(id);
  }

  createGroup(): Promise<GroupIdHex> {
    return this.native.createGroup();
  }

  groupInvite(gid: GroupIdHex, peer: NodeIdHex): Promise<void> {
    return this.native.groupInvite(gid, peer);
  }

  groupSend(gid: GroupIdHex, text: string): Promise<MsgIdHex> {
    return this.native.groupSend(gid, text);
  }

  groupMembers(gid: GroupIdHex): Promise<NodeIdHex[]> {
    return this.native.groupMembers(gid);
  }

  groupRemove(gid: GroupIdHex, member: NodeIdHex): Promise<number> {
    return this.native.groupRemove(gid, member);
  }

  setNickname(name: string, seq: number): Promise<void> {
    return this.native.setNickname(name, seq);
  }

  ownClaims(): Promise<string[]> {
    return this.native.ownClaims();
  }

  ingestClaim(claimB64: string, verified: boolean): Promise<void> {
    return this.native.ingestClaim(claimB64, verified);
  }

  revokeIdentity(reason: number): Promise<string> {
    return this.native.revokeIdentity(reason);
  }

  tryRevokeIdentity(reason: number): Promise<string> {
    return this.native.tryRevokeIdentity(reason);
  }

  revokeIdentityWithSuccessor(reason: number, successorPublicKeyB64: string, countersigB64: string): Promise<string> {
    return this.native.revokeIdentityWithSuccessor(reason, successorPublicKeyB64, countersigB64);
  }

  block(peer: NodeIdHex): Promise<void> {
    return this.native.block(peer);
  }

  enableDiagnostics(): Promise<void> {
    return this.native.enableDiagnostics();
  }

  diagDump(): Promise<string> {
    return this.native.diagDump();
  }

  storeLen(): Promise<number> {
    return this.native.storeLen();
  }

  /** Force an immediate persist; native lifecycle hooks also call this on background. */
  flush(): Promise<void> {
    return this.native.flush();
  }

  lastPersistError(): Promise<string | null> {
    return this.native.lastPersistError();
  }

  /** Subscribe only to app-level events; raw BLE frames never cross this boundary. */
  onEvent(handler: (event: MmpEvent) => void): () => void {
    const subscription = this.emitter.addListener('mmp', handler);
    return () => subscription.remove();
  }
}

export function createMmpClient(nativeModule?: unknown, eventEmitter?: MmpEventEmitter): MmpClient {
  return new MmpClient(nativeModule, eventEmitter);
}

export const mmp = new MmpClient();
