package org.meshmilk.platform

/**
 * Process-local connection-token source for native BLE adapters.
 *
 * A platform connection handle can be reused while callbacks for an earlier
 * connection are still queued.  Adapters must therefore pass a fresh token
 * to every [MeshDrive] callback instead of forwarding a raw handle.  Tokens
 * are positive, never reused, and fail closed before the signed `Long` range
 * could wrap.  The synchronized allocator also tolerates a platform that
 * creates connections from more than one callback queue.
 */
public class LinkTokenAllocator(firstToken: Long = 1L) {
    init {
        require(firstToken > 0L) { "link tokens must be positive" }
    }

    private var nextToken: Long = firstToken

    /** Return one token that must remain associated with one physical link. */
    @Synchronized
    public fun allocate(): Long {
        val token = nextToken
        if (token == Long.MAX_VALUE) {
            throw IllegalStateException("link token space exhausted")
        }
        nextToken = token + 1L
        return token
    }
}
