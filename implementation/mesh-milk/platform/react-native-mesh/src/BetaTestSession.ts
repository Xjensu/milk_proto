import type {
  Capabilities,
  MmpEvent,
  MmpClient,
  NodeIdHex,
  NodeMetrics,
  NodeStatus,
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
    this.unsubscribe = this.client.onEvent((event) => {
      if (this.events.length >= this.maxEvents) {
        this.events.shift();
        this.droppedEvents += 1;
      }
      this.events.push(event);
    });
    try {
      const id = await this.client.start(tier);
      this.started = true;
      return id;
    } catch (error) {
      this.unsubscribe?.();
      this.unsubscribe = null;
      throw error;
    }
  }

  /** Stop the native mesh and detach the local event listener. */
  async stop(): Promise<void> {
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
}
