package org.meshmilk.platform

import java.util.IdentityHashMap

/** Opaque identity for one physical connection instance; never reuse it. */
public class ConnectionIdentity private constructor() {
    public companion object {
        /** Create the identity object captured by one connection's callbacks. */
        public fun create(): ConnectionIdentity = ConnectionIdentity()
    }
}

/**
 * Binds one native connection-instance identity to one [MeshDrive] link token.
 *
 * Android connection handles and Bluetooth addresses can be reused before all
 * callbacks for a retired connection have drained.  The native adapter must
 * create a fresh identity object per physical connection and capture the
 * returned [ConnectionBinding] in every callback closure.  This registry never
 * resolves callbacks from a raw handle, so a delayed callback from an old
 * connection cannot retire or feed a replacement connection.
 */
public class ConnectionBindingRegistry(
    private val maxBindings: Int = 64,
    private val allocator: LinkTokenAllocator = LinkTokenAllocator(),
) {
    init {
        require(maxBindings > 0) { "maxBindings must be positive" }
    }

    private val active = IdentityHashMap<ConnectionIdentity, ConnectionBinding>()

    /** Immutable callback context for one physical connection instance. */
    public class ConnectionBinding internal constructor(
        internal val identity: ConnectionIdentity,
        public val link: Long,
    )

    /**
     * Admit a connection identity, returning its existing binding for a
     * duplicate callback.  Capacity is checked only after duplicate lookup.
     */
    @Synchronized
    public fun open(identity: ConnectionIdentity): ConnectionBinding? {
        active[identity]?.let { return it }
        if (active.size >= maxBindings) return null
        val binding = ConnectionBinding(identity, allocator.allocate())
        active[identity] = binding
        return binding
    }

    /** Retire a binding only if it is still the current identity's binding. */
    @Synchronized
    public fun close(binding: ConnectionBinding): Boolean {
        val current = active[binding.identity]
        if (current !== binding) return false
        active.remove(binding.identity)
        return true
    }

    /** Return whether a callback context still owns a live connection. */
    @Synchronized
    public fun isCurrent(binding: ConnectionBinding): Boolean {
        return active[binding.identity] === binding
    }

    /** Number of live connection instances currently admitted. */
    @Synchronized
    public fun size(): Int = active.size
}
