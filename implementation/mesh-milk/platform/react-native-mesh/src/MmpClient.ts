import { NativeEventEmitter, NativeModules } from 'react-native';

// ---- wire types (mirror the FFI surface; keep in sync with mmp-ffi) --------

export type NodeIdHex = string;   // 32 lowercase hex chars (16 bytes)
export type MsgIdHex = string;    // 32 lowercase hex chars (16 bytes)
export type GroupIdHex = string;  // 32 lowercase hex chars (16 bytes)
export type ScopeIdHex = string;  // 32 lowercase hex chars (16 bytes)
export type FileRootHex = string; // 32 lowercase hex chars (16-byte Merkle root)
export type Base64Value = string; // variable-length opaque bytes
/** Base64-encoded 32-byte Ed25519 public key; never a private key. */
export type PublicKeyB64 = Base64Value;
/** Base64-encoded 64-byte Ed25519 signature. */
export type SignatureB64 = Base64Value;

export type Tier = 'leaf' | 'relay' | 'anchor';

/**
 * Native UniFFI spellings for the relay tier enum.  The Rust FFI exposes
 * `FfiTier::{Leaf, Relay, Anchor}`; generated Kotlin bindings may use upper
 * snake case.  Keep this conversion explicit so native enum naming never
 * leaks into the application-facing contract.
 */
export type NativeTier =
  | 'Leaf' | 'Relay' | 'Anchor'
  | 'LEAF' | 'RELAY' | 'ANCHOR'
  | 'leaf' | 'relay' | 'anchor';

/** Normalize a native tier and reject unknown values instead of guessing. */
export function mapNativeTier(value: unknown): Tier {
  switch (value) {
    case 'Leaf':
    case 'LEAF':
    case 'leaf':
      return 'leaf';
    case 'Relay':
    case 'RELAY':
    case 'relay':
      return 'relay';
    case 'Anchor':
    case 'ANCHOR':
    case 'anchor':
      return 'anchor';
    default:
      throw new MmpNativeModuleError(`Unknown native tier: ${String(value)}`);
  }
}

function mapNativeStatus(value: NodeStatus): NodeStatus {
  return { ...value, tier: mapNativeTier(value.tier) };
}

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

export type NativeCapability =
  | 'DirectMessaging' | 'DIRECT_MESSAGING' | 'directMessaging' | 'direct_messaging'
  | 'GroupMessaging' | 'GROUP_MESSAGING' | 'groupMessaging' | 'group_messaging'
  | 'FileTransfer' | 'FILE_TRANSFER' | 'fileTransfer' | 'file_transfer'
  | 'Announcements' | 'ANNOUNCEMENTS' | 'announcements'
  | 'Diagnostics' | 'DIAGNOSTICS' | 'diagnostics'
  | 'TransportInjection' | 'TRANSPORT_INJECTION' | 'transportInjection' | 'transport_injection';

export type Capability =
  | 'directMessaging'
  | 'groupMessaging'
  | 'fileTransfer'
  | 'announcements'
  | 'diagnostics'
  | 'transportInjection';

export function mapNativeCapability(value: unknown): Capability {
  switch (value) {
    case 'DirectMessaging': case 'DIRECT_MESSAGING': case 'directMessaging': case 'direct_messaging': return 'directMessaging';
    case 'GroupMessaging': case 'GROUP_MESSAGING': case 'groupMessaging': case 'group_messaging': return 'groupMessaging';
    case 'FileTransfer': case 'FILE_TRANSFER': case 'fileTransfer': case 'file_transfer': return 'fileTransfer';
    case 'Announcements': case 'ANNOUNCEMENTS': case 'announcements': return 'announcements';
    case 'Diagnostics': case 'DIAGNOSTICS': case 'diagnostics': return 'diagnostics';
    case 'TransportInjection': case 'TRANSPORT_INJECTION': case 'transportInjection': case 'transport_injection': return 'transportInjection';
    default: throw new MmpNativeModuleError(`Unknown native capability: ${String(value)}`);
  }
}

/** Stable capability report for beta-test UI; persistence is node-specific. */
export interface Capabilities {
  capabilities: Capability[];
  persistentStore: boolean;
}

export interface NativeCapabilities {
  capabilities: NativeCapability[];
  persistentStore: boolean;
}

/** Native versioned envelope for a secret-free beta-test report. */
export interface NativeBetaSnapshot {
  schemaVersion: number;
  status: NodeStatus;
  metrics: NodeMetrics;
  capabilities: NativeCapabilities;
}

/** Versioned, normalized beta-test report exposed to applications. */
export interface BetaSnapshot {
  schemaVersion: 1;
  status: NodeStatus;
  metrics: NodeMetrics;
  capabilities: Capabilities;
}

/**
 * Normalize the native beta report without accepting an unknown report
 * version.  The schema version is an API compatibility fence, not a wire
 * protocol version; callers must fail closed until they understand a newer
 * native envelope.
 */
export function mapNativeBetaSnapshot(value: NativeBetaSnapshot): BetaSnapshot {
  if (value.schemaVersion !== 1) {
    throw new MmpNativeModuleError(`Unknown native beta snapshot schema: ${String(value.schemaVersion)}`);
  }
  return {
    schemaVersion: 1,
    status: mapNativeStatus(value.status),
    metrics: value.metrics,
    capabilities: {
      capabilities: value.capabilities.capabilities.map((capability) => mapNativeCapability(capability)),
      persistentStore: value.capabilities.persistentStore,
    },
  };
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

function requireNativeString(value: unknown, field: string): string {
  if (typeof value !== 'string') {
    throw new MmpNativeModuleError(`Malformed native file event field: ${field}`);
  }
  return value;
}

/**
 * Normalize one native file offer without changing opaque encodings.  Some
 * native bridges omit an absent UniFFI Option instead of serializing it as
 * null; the public API deliberately exposes a stable nullable field.
 */
export function mapNativeFileOffer(value: unknown): FileOffer {
  if (value === null || typeof value !== 'object') {
    throw new MmpNativeModuleError('Malformed native file offer');
  }
  const raw = value as Record<string, unknown>;
  const size = raw.size;
  if (typeof size !== 'number' || !Number.isSafeInteger(size) || size < 0) {
    throw new MmpNativeModuleError('Malformed native file offer field: size');
  }
  const thumbRoot = raw.thumbRoot;
  return {
    nameB64: requireNativeString(raw.nameB64, 'nameB64'),
    mimeB64: requireNativeString(raw.mimeB64, 'mimeB64'),
    size,
    root: requireNativeString(raw.root, 'root'),
    thumbRoot: thumbRoot === undefined || thumbRoot === null
      ? null
      : requireNativeString(thumbRoot, 'thumbRoot'),
  };
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
  | { type: 'friendshipEstablished'; peer: NodeIdHex; since: number }
  | { type: 'peerRevoked'; peer: NodeIdHex; successor: string | null }
  | { type: 'contactSeen'; peer: NodeIdHex }
  | { type: 'established'; peer: NodeIdHex }
  | { type: 'groupInvite'; gid: GroupIdHex; from: NodeIdHex }
  | { type: 'groupMessage'; gid: GroupIdHex; from: NodeIdHex; id: MsgIdHex; apptype: number; body: string }
  | { type: 'groupEpochChanged'; gid: GroupIdHex; epoch: number }
  | { type: 'tierChanged'; tier: Tier }
  | {
      type: 'announcement';
      id: MsgIdHex;
      scope: ScopeIdHex;
      annType: number;
      notAfter: number;
      text: Base64Value;
      from: NodeIdHex;
      official: boolean;
      effectivePriority: number;
      scopeNameHint: Base64Value | null;
    }
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
  capabilities(): Promise<NativeCapabilities>;
  betaSnapshot(): Promise<NativeBetaSnapshot>;

  // Authority, clock, and scope controls. These values are validated by the
  // native FFI adapter; opaque key/certificate bytes use base64.
  setWallClock(unixS: number): Promise<void>;
  pinAuthorityRoot(scope: ScopeIdHex, issuerPublicKeyB64: PublicKeyB64): Promise<void>;
  subscribeScope(scopeName: string): Promise<ScopeIdHex>;
  unsubscribeScope(scope: ScopeIdHex): Promise<void>;
  mintAuthCert(
    suite: number,
    subjectPublicKeyB64: PublicKeyB64,
    scope: ScopeIdHex,
    caps: number,
    priorityBudget: number,
    notAfter: number,
  ): Promise<Base64Value>;
  publishAnnouncement(
    scope: ScopeIdHex,
    annType: number,
    priority: number,
    text: string,
    certB64: Base64Value | null,
  ): Promise<MsgIdHex>;

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
  requestFriendship(peerPublicKeyB64: PublicKeyB64): Promise<void>;
  allowFriendshipFrom(peerPublicKeyB64: PublicKeyB64): Promise<void>;

  // Messaging.
  sendText(peer: NodeIdHex, text: string): Promise<MsgIdHex>;
  /** Send an application-defined body; payload bytes are base64 at the JS boundary. */
  sendBody(peer: NodeIdHex, apptype: number, payloadB64: Base64Value): Promise<MsgIdHex>;
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
  groupSendBody(gid: GroupIdHex, apptype: number, payloadB64: Base64Value): Promise<MsgIdHex>;
  groupMembers(gid: GroupIdHex): Promise<NodeIdHex[]>;
  groupRemove(gid: GroupIdHex, member: NodeIdHex): Promise<number>;

  // Diagnostics.
  enableDiagnostics(): Promise<void>;
  disableDiagnostics(): Promise<void>;
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
  'capabilities',
  'betaSnapshot',
  'setWallClock',
  'pinAuthorityRoot',
  'subscribeScope',
  'unsubscribeScope',
  'mintAuthCert',
  'publishAnnouncement',
  'nodeId',
  'publicKey',
  'ownClaims',
  'ingestClaim',
  'setNickname',
  'revokeIdentity',
  'tryRevokeIdentity',
  'revokeIdentityWithSuccessor',
  'block',
  'requestFriendship',
  'allowFriendshipFrom',
  'sendText',
  'sendBody',
  'messageState',
  'sendFile',
  'fetchFile',
  'cancelFetch',
  'fileProgress',
  'createGroup',
  'groupInvite',
  'groupSend',
  'groupSendBody',
  'groupMembers',
  'groupRemove',
  'enableDiagnostics',
  'disableDiagnostics',
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
    return this.native.status().then((value) => mapNativeStatus(value));
  }

  metrics(): Promise<NodeMetrics> {
    return this.native.metrics();
  }

  capabilities(): Promise<Capabilities> {
    return this.native.capabilities().then((report) => ({
      capabilities: report.capabilities.map((value) => mapNativeCapability(value)),
      persistentStore: report.persistentStore,
    }));
  }

  /** Return the native versioned secret-free report with an explicit schema fence. */
  betaSnapshot(): Promise<BetaSnapshot> {
    return this.native.betaSnapshot().then((value) => mapNativeBetaSnapshot(value));
  }

  setWallClock(unixS: number): Promise<void> {
    return this.native.setWallClock(unixS);
  }

  pinAuthorityRoot(scope: ScopeIdHex, issuerPublicKeyB64: PublicKeyB64): Promise<void> {
    return this.native.pinAuthorityRoot(scope, issuerPublicKeyB64);
  }

  subscribeScope(scopeName: string): Promise<ScopeIdHex> {
    return this.native.subscribeScope(scopeName);
  }

  unsubscribeScope(scope: ScopeIdHex): Promise<void> {
    return this.native.unsubscribeScope(scope);
  }

  mintAuthCert(
    suite: number,
    subjectPublicKeyB64: PublicKeyB64,
    scope: ScopeIdHex,
    caps: number,
    priorityBudget: number,
    notAfter: number,
  ): Promise<Base64Value> {
    return this.native.mintAuthCert(suite, subjectPublicKeyB64, scope, caps, priorityBudget, notAfter);
  }

  publishAnnouncement(
    scope: ScopeIdHex,
    annType: number,
    priority: number,
    text: string,
    certB64: Base64Value | null = null,
  ): Promise<MsgIdHex> {
    return this.native.publishAnnouncement(scope, annType, priority, text, certB64);
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

  sendBody(peer: NodeIdHex, apptype: number, payloadB64: Base64Value): Promise<MsgIdHex> {
    return this.native.sendBody(peer, apptype, payloadB64);
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

  groupSendBody(gid: GroupIdHex, apptype: number, payloadB64: Base64Value): Promise<MsgIdHex> {
    return this.native.groupSendBody(gid, apptype, payloadB64);
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

  requestFriendship(peerPublicKeyB64: PublicKeyB64): Promise<void> {
    return this.native.requestFriendship(peerPublicKeyB64);
  }

  allowFriendshipFrom(peerPublicKeyB64: PublicKeyB64): Promise<void> {
    return this.native.allowFriendshipFrom(peerPublicKeyB64);
  }

  enableDiagnostics(): Promise<void> {
    return this.native.enableDiagnostics();
  }

  disableDiagnostics(): Promise<void> {
    return this.native.disableDiagnostics();
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
        if (event.type === 'fileFailed') {
          // UniFFI emits the native enum spelling (HashMismatch/Abandoned),
          // while the JS contract intentionally uses camel-case values.
          // Normalize at the bridge boundary so beta applications never
          // have to handle two incompatible event schemas.
          handler({
            ...event,
            reason: mapNativeFileFailReason((event as unknown as { reason: unknown }).reason),
          });
          return;
        }
        if (event.type === 'fileOffered') {
          const entries = (event as unknown as { entries: unknown }).entries;
          if (!Array.isArray(entries)) {
            throw new MmpNativeModuleError('Malformed native fileOffered entries');
          }
          handler({
            ...event,
            entries: entries.map((entry) => mapNativeFileOffer(entry)),
          });
          return;
        }
        if (event.type === 'fileReady') {
          const ready = event as unknown as { root: unknown; path: unknown };
          handler({
            ...event,
            root: requireNativeString(ready.root, 'root'),
            path: requireNativeString(ready.path, 'path'),
          });
          return;
        }
        if (event.type === 'tierChanged') {
          handler({
            ...event,
            tier: mapNativeTier((event as unknown as { tier: unknown }).tier),
          });
          return;
        }
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
