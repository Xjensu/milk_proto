import { NativeEventEmitter, NativeModules } from 'react-native';

// ---- wire types (mirror the FFI surface; keep in sync with mmp-ffi) --------

export type NodeIdHex = string;   // 32 lowercase hex chars (16 bytes)
export type MsgIdHex = string;    // 32 lowercase hex chars (16 bytes)
export type GroupIdHex = string;  // 32 lowercase hex chars (16 bytes)
export type FileRootHex = string; // 32 lowercase hex chars (16-byte Merkle root)
export type Base64Value = string; // variable-length opaque bytes

export type Tier = 'leaf' | 'relay' | 'anchor';

/** Secret-free lifecycle snapshot exposed to beta applications and harnesses. */
export interface NodeStatus {
  tier: Tier;
  persistentStore: boolean;
  diagnosticsEnabled: boolean;
  lastTickMs: number;
}

/** Local bounded counters for diagnostics; identifiers and key material are excluded. */
export interface NodeMetrics {
  storeLen: number;
  closeActionsDropped: number;
  eventQueueDropped: number;
  diagnosticRecords: number;
  diagnosticClamps: number;
  diagnosticDrops: number;
}

export type DeliveryState = 'queued' | 'inMesh' | 'delivered' | 'expired';

/**
 * Native UniFFI spellings for a delivery-state enum.  The bridge accepts the
 * canonical Rust names and the historical lower-case spelling emitted by the
 * first mobile adapters; all values are converted explicitly before they are
 * exposed to JavaScript.
 */
export type NativeDeliveryState =
  | 'Queued'
  | 'InMesh'
  | 'Delivered'
  | 'Expired'
  | 'QUEUED'
  | 'IN_MESH'
  | 'DELIVERED'
  | 'EXPIRED'
  | 'queued'
  | 'inMesh'
  | 'inmesh'
  | 'in_mesh'
  | 'delivered'
  | 'expired';

/**
 * Map the native enum without relying on generic case conversion.  Unknown
 * values fail closed so an adapter drift cannot silently change UI state.
 */
export function mapNativeDeliveryState(value: unknown): DeliveryState {
  switch (value) {
    case 'Queued':
    case 'QUEUED':
    case 'queued':
      return 'queued';
    case 'InMesh':
    case 'IN_MESH':
    case 'inMesh':
    case 'inmesh':
    case 'in_mesh':
      return 'inMesh';
    case 'Delivered':
    case 'DELIVERED':
    case 'delivered':
      return 'delivered';
    case 'Expired':
    case 'EXPIRED':
    case 'expired':
      return 'expired';
    default:
      throw new MmpNativeModuleError(`Unknown native delivery state: ${String(value)}`);
  }
}

export type BlobState = 'wanted' | 'fetching' | 'assembled' | 'abandoned';
export type FileFailReason = 'hashMismatch' | 'abandoned';

/**
 * Raw UniFFI spellings received by a native event adapter.  These values are
 * intentionally kept separate from the JavaScript event spelling: converting
 * an enum by generic lower-casing would produce `hashmismatch`, which is not a
 * member of `FileFailReason`.
 */
export type NativeFileFailReason = 'HashMismatch' | 'Abandoned';

/**
 * Convert the native enum explicitly before constructing a `fileFailed` event.
 * Native adapters must use this mapping (or an equivalent exhaustive switch),
 * never a generic case conversion.
 */
export function mapNativeFileFailReason(value: unknown): FileFailReason {
  switch (value) {
    case 'HashMismatch':
      return 'hashMismatch';
    case 'Abandoned':
      return 'abandoned';
    default:
      throw new MmpNativeModuleError(`Unknown native file failure reason: ${String(value)}`);
  }
}

/** Result of accepting a file for sharing. IDs remain lowercase hex strings. */
export interface FileSend {
  messageId: MsgIdHex;
  root: FileRootHex;
}

/** Secret-free progress snapshot for one explicitly requested file. */
export interface FileProgress {
  state: BlobState;
  haveSymbols: number;
  totalSymbols: number;
  stripesDone: number;
  stripes: number;
}

/** A verified file offer; name and MIME bytes retain the bridge's base64 convention. */
export interface FileOffer {
  nameB64: Base64Value;
  mimeB64: Base64Value;
  size: number;
  root: FileRootHex;
  thumbRoot: FileRootHex | null;
}

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
  | { type: 'tierChanged'; tier: Tier }
  | { type: 'fileOffered'; conv: MsgIdHex; from: NodeIdHex; sharer: NodeIdHex; entries: FileOffer[] }
  | { type: 'fileReady'; root: FileRootHex; path: string }
  | { type: 'fileFailed'; root: FileRootHex; reason: FileFailReason };

/** The native methods consumed by this package. Link bytes are intentionally absent. */
export interface MmpMeshNative {
  // Lifecycle. Idempotent: if a node already exists for this seed/store, returns its id.
  start(tier: Tier): Promise<NodeIdHex>;
  stop(): Promise<void>;
  setTier(tier: Tier): Promise<void>;
  status(): Promise<NodeStatus>;
  metrics(): Promise<NodeMetrics>;

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
  /** Returns a native enum string; MmpClient validates and normalizes it. */
  messageState(id: MsgIdHex): Promise<string | null>;

  // File transfer. Data and variable-length manifest fields use base64;
  // content-addressed IDs use lowercase hex. Full files are explicit-pull.
  sendFile(peer: NodeIdHex, name: string, mime: string, dataB64: Base64Value): Promise<FileSend>;
  fetchFile(root: FileRootHex, dest: string): Promise<void>;
  cancelFetch(root: FileRootHex): Promise<void>;
  fileProgress(root: FileRootHex): Promise<FileProgress | null>;

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
  'status',
  'metrics',
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
  'sendFile',
  'fetchFile',
  'cancelFetch',
  'fileProgress',
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

  status(): Promise<NodeStatus> {
    return this.native.status();
  }

  metrics(): Promise<NodeMetrics> {
    return this.native.metrics();
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
    return this.native.messageState(id).then((value) =>
      value === null ? null : mapNativeDeliveryState(value),
    );
  }

  /** Share a file; `dataB64` is decoded by the native adapter, never by the core API. */
  sendFile(peer: NodeIdHex, name: string, mime: string, dataB64: Base64Value): Promise<FileSend> {
    return this.native.sendFile(peer, name, mime, dataB64);
  }

  /** Explicitly request a verified offer and write the assembled file to `dest`. */
  fetchFile(root: FileRootHex, dest: string): Promise<void> {
    return this.native.fetchFile(root, dest);
  }

  cancelFetch(root: FileRootHex): Promise<void> {
    return this.native.cancelFetch(root);
  }

  fileProgress(root: FileRootHex): Promise<FileProgress | null> {
    return this.native.fileProgress(root);
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
    const subscription = this.emitter.addListener('mmp', (event) => {
      if (event.type !== 'deliveryProgress') {
        handler(event);
        return;
      }

      // Native event adapters may still provide Rust/legacy enum spellings;
      // normalize only this event and preserve every unrelated payload byte.
      handler({
        ...event,
        state: mapNativeDeliveryState((event as unknown as { state: unknown }).state),
      });
    });
    return () => subscription.remove();
  }
}

export function createMmpClient(nativeModule?: unknown, eventEmitter?: MmpEventEmitter): MmpClient {
  return new MmpClient(nativeModule, eventEmitter);
}

export const mmp = new MmpClient();
