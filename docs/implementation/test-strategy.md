# Test strategy

Run wire and crypto tests on every change to `mmp-types`, `mmp-crypto`, or `mmp-core`. Regenerate vectors and require a byte-for-byte drift check. Exercise negative inputs before authenticated state. Run state-machine tests with timer and link-loss injections. Run persistence tests through crash and recovery paths.

The fuzz targets cover beacon, envelope, bundle, and Noise handshake parsing. A build of a fuzz target is a link gate; it is not a fuzzing campaign. The current Windows environment cannot link the sanitizer runtime, so the retained CI run is required.

Transport tests must separate codec success from hardware evidence. A release record must state whether a result is simulated, loopback, Linux/BlueZ, Android, iOS, or embedded hardware. The absence of a platform must be recorded as an environment limitation rather than converted into a code change.
