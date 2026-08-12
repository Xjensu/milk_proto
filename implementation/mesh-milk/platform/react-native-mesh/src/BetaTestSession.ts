import type {
  Base64Value,
  BetaSnapshot,
  Capabilities,
  DeliveryState,
  FileProgress,
  FileRootHex,
  FileSend,
  GroupIdHex,
  MmpEvent,
  MmpClient,
  MsgIdHex,
  NodeIdHex,
  NodeMetrics,
  NodeStatus,
  PublicKeyB64,
  ScopeIdHex,
  Tier,
} from './MmpClient';

/** Construction options for the local beta-test harness. */
export interface BetaTestSessionOptions {
  /** Maximum number of most-recent app events retained locally. */
  maxEvents?: number;
}

/** A deterministic, local-only snapshot suitable for a beta-test report. */
export interface BetaTestReport {
  status: NodeStatus;
  metrics: NodeMetrics;
  capabilities: Capabilities;
  events: MmpEvent[];
  droppedEvents: number;
}

/** Error raised when an app tries to exercise an active-session operation too early. */
export class BetaTestSessionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'BetaTestSessionError';
  }
}

/**
 * Small application-side harness for manual and beta testing.
 *
 * It owns no native resources and changes no wire behavior: the supplied
 * `MmpClient` remains the sole owner of the native module. Event history is
 * bounded and local; reports never upload or include identity seeds, private
 * keys, provider handles, or raw transport bytes.
 */
export class BetaTestSession {
  private readonly client: MmpClient;
  private readonly maxEvents: number;
  private events: MmpEvent[] = [];
  private droppedEvents = 0;
  private unsubscribe: (() => void) | null = null;
  private started = false;
  private startPromise: Promise<NodeIdHex> | null = null;

  constructor(client: MmpClient, options: BetaTestSessionOptions = {}) {
    const maxEvents = options.maxEvents ?? 128;
    if (!Number.isSafeInteger(maxEvents) || maxEvents < 1 || maxEvents > 4096) {
      throw new RangeError('maxEvents must be a safe integer between 1 and 4096');
    }
    this.client = client;
    this.maxEvents = maxEvents;
  }

  /** Start the native mesh and begin capturing app-level events. */
  async start(tier: Tier = 'leaf'): Promise<NodeIdHex> {
    if (this.started) {
      return this.client.nodeId();
    }
    if (this.startPromise !== null) {
      return this.startPromise;
    }
    this.unsubscribe = this.client.onEvent((event) => {
      if (this.events.length >= this.maxEvents) {
        this.events.shift();
        this.droppedEvents += 1;
      }
      this.events.push(event);
    });
    const pending = Promise.resolve().then(() => this.client.start(tier)).then(
      (id) => {
        this.started = true;
        return id;
      },
      (error) => {
        this.unsubscribe?.();
        this.unsubscribe = null;
        throw error;
      },
    );
    this.startPromise = pending;
    try {
      return await pending;
    } finally {
      if (this.startPromise === pending) {
        this.startPromise = null;
      }
    }
  }

  /** Stop the native mesh and detach the local event listener. */
  async stop(): Promise<void> {
    if (this.startPromise !== null) {
      try {
        await this.startPromise;
      } catch {
        return;
      }
    }
    if (!this.started) {
      return;
    }
    try {
      await this.client.stop();
    } finally {
      this.unsubscribe?.();
      this.unsubscribe = null;
      this.started = false;
    }
  }

  /** Explicitly persist the native store; never called implicitly by this helper. */
  flush(): Promise<void> {
    return this.client.flush();
  }

  /** Return the current secret-free status without requiring the mesh to run. */
  status(): Promise<NodeStatus> {
    return this.client.status();
  }

  /**
   * Clear the local event history at a test-case boundary.
   *
   * This does not stop the native mesh, flush storage, or acknowledge any
   * protocol event; it only resets the bounded report window and its local
   * drop counter.
   */
  clearHistory(): void {
    this.events.length = 0;
    this.droppedEvents = 0;
  }

  /** Return native diagnostic counters without exposing transport or key material. */
  metrics(): Promise<NodeMetrics> {
    return this.client.metrics();
  }

  /** Change the local routing/storage tier for an active beta session. */
  setTier(tier: Tier): Promise<void> {
    this.requireStarted();
    return this.client.setTier(tier);
  }

  /** Return the fail-closed, normalized native capability report. */
  capabilities(): Promise<Capabilities> {
    return this.client.capabilities();
  }

  /** Return the versioned secret-free native snapshot for a beta test step. */
  betaSnapshot(): Promise<BetaSnapshot> {
    return this.client.betaSnapshot();
  }

  /** Send a direct text message through the native MMP/1 implementation. */
  sendText(peer: NodeIdHex, text: string): Promise<MsgIdHex> {
    this.requireStarted();
    return this.client.sendText(peer, text);
  }

  /** Send an application-defined body through the running MMP/1 session. */
  sendBody(peer: NodeIdHex, apptype: number, payloadB64: Base64Value): Promise<MsgIdHex> {
    this.requireStarted();
    return this.client.sendBody(peer, apptype, payloadB64);
  }

  /** Inspect native delivery state; unknown enum values fail in MmpClient. */
  messageState(id: MsgIdHex): Promise<DeliveryState | null> {
    this.requireStarted();
    return this.client.messageState(id);
  }

  /** Share a bounded file payload; file bytes remain an explicit app input. */
  sendFile(peer: NodeIdHex, name: string, mime: string, dataB64: Base64Value): Promise<FileSend> {
    this.requireStarted();
    return this.client.sendFile(peer, name, mime, dataB64);
  }

  /** Explicitly pull a verified file offer to an app-selected destination. */
  fetchFile(root: FileRootHex, destination: string): Promise<void> {
    this.requireStarted();
    return this.client.fetchFile(root, destination);
  }

  cancelFetch(root: FileRootHex): Promise<void> {
    this.requireStarted();
    return this.client.cancelFetch(root);
  }

  fileProgress(root: FileRootHex): Promise<FileProgress | null> {
    this.requireStarted();
    return this.client.fileProgress(root);
  }

  /** Create a group and exercise the existing sender-key/group API. */
  createGroup(): Promise<GroupIdHex> {
    this.requireStarted();
    return this.client.createGroup();
  }

  groupInvite(gid: GroupIdHex, peer: NodeIdHex): Promise<void> {
    this.requireStarted();
    return this.client.groupInvite(gid, peer);
  }

  groupSend(gid: GroupIdHex, text: string): Promise<MsgIdHex> {
    this.requireStarted();
    return this.client.groupSend(gid, text);
  }

  groupMembers(gid: GroupIdHex): Promise<NodeIdHex[]> {
    this.requireStarted();
    return this.client.groupMembers(gid);
  }

  groupRemove(gid: GroupIdHex, member: NodeIdHex): Promise<number> {
    this.requireStarted();
    return this.client.groupRemove(gid, member);
  }

  /** Set local authority time; this never establishes trusted network time. */
  setWallClock(unixS: number): Promise<void> {
    this.requireStarted();
    return this.client.setWallClock(unixS);
  }

  pinAuthorityRoot(scope: ScopeIdHex, issuerPublicKeyB64: PublicKeyB64): Promise<void> {
    this.requireStarted();
    return this.client.pinAuthorityRoot(scope, issuerPublicKeyB64);
  }

  subscribeScope(scopeName: string): Promise<ScopeIdHex> {
    this.requireStarted();
    return this.client.subscribeScope(scopeName);
  }

  unsubscribeScope(scope: ScopeIdHex): Promise<void> {
    this.requireStarted();
    return this.client.unsubscribeScope(scope);
  }

  mintAuthCert(
    suite: number,
    subjectPublicKeyB64: PublicKeyB64,
    scope: ScopeIdHex,
    caps: number,
    priorityBudget: number,
    notAfter: number,
  ): Promise<Base64Value> {
    this.requireStarted();
    return this.client.mintAuthCert(suite, subjectPublicKeyB64, scope, caps, priorityBudget, notAfter);
  }

  publishAnnouncement(
    scope: ScopeIdHex,
    annType: number,
    priority: number,
    text: string,
    certB64: Base64Value | null = null,
  ): Promise<MsgIdHex> {
    this.requireStarted();
    return this.client.publishAnnouncement(scope, annType, priority, text, certB64);
  }

  /** Public identity and claims are safe app-facing values; private key material is not exposed. */
  nodeId(): Promise<NodeIdHex> {
    return this.client.nodeId();
  }

  publicKey(): Promise<string> {
    return this.client.publicKey();
  }

  ownClaims(): Promise<string[]> {
    this.requireStarted();
    return this.client.ownClaims();
  }

  ingestClaim(claimB64: string, verified: boolean): Promise<void> {
    this.requireStarted();
    return this.client.ingestClaim(claimB64, verified);
  }

  setNickname(name: string, seq: number): Promise<void> {
    this.requireStarted();
    return this.client.setNickname(name, seq);
  }

  revokeIdentity(reason: number): Promise<string> {
    this.requireStarted();
    return this.client.revokeIdentity(reason);
  }

  tryRevokeIdentity(reason: number): Promise<string> {
    this.requireStarted();
    return this.client.tryRevokeIdentity(reason);
  }

  revokeIdentityWithSuccessor(reason: number, successorPublicKeyB64: string, countersigB64: string): Promise<string> {
    this.requireStarted();
    return this.client.revokeIdentityWithSuccessor(reason, successorPublicKeyB64, countersigB64);
  }

  block(peer: NodeIdHex): Promise<void> {
    this.requireStarted();
    return this.client.block(peer);
  }

  requestFriendship(peerPublicKeyB64: PublicKeyB64): Promise<void> {
    this.requireStarted();
    return this.client.requestFriendship(peerPublicKeyB64);
  }

  allowFriendshipFrom(peerPublicKeyB64: PublicKeyB64): Promise<void> {
    this.requireStarted();
    return this.client.allowFriendshipFrom(peerPublicKeyB64);
  }

  enableDiagnostics(): Promise<void> {
    this.requireStarted();
    return this.client.enableDiagnostics();
  }

  diagDump(): Promise<string> {
    this.requireStarted();
    return this.client.diagDump();
  }

  storeLen(): Promise<number> {
    return this.client.storeLen();
  }

  lastPersistError(): Promise<string | null> {
    return this.client.lastPersistError();
  }

  /** Return a secret-free status/metrics/capability snapshot and local events. */
  async report(): Promise<BetaTestReport> {
    const [status, metrics, capabilities] = await Promise.all([
      this.client.status(),
      this.client.metrics(),
      this.client.capabilities(),
    ]);
    return {
      status,
      metrics,
      capabilities,
      events: this.events.slice(),
      droppedEvents: this.droppedEvents,
    };
  }

  /** Serialize `report()` with stable object insertion order and no upload. */
  async reportJson(): Promise<string> {
    return JSON.stringify(await this.report());
  }

  /** Whether this harness currently owns an active native lifecycle. */
  get isStarted(): boolean {
    return this.started;
  }

  private requireStarted(): void {
    if (!this.started) {
      throw new BetaTestSessionError('BetaTestSession is not started; call start() before exercising the mesh API');
    }
  }
}
