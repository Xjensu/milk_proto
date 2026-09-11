# Group messaging extension

Group messages use per-group, per-epoch sender keys distributed over pairwise encrypted conversations. A sender advances its symmetric chain for every message and signs the group ciphertext with a group-specific signing key. The message identifies group, epoch, sender member, and chain index.

An effective membership change requires a new epoch. Removed members MUST NOT receive the new epoch's sender keys. A late joiner receives keys for the current epoch after admission and does not receive historical keys by default.

The current implementation has sender-key encryption, member signatures, skipped-key handling, and epoch state. It does not yet provide the final authenticated administrator-anchor rule for future epochs or a bounded late-join control schema. Future-epoch distributions can still pass through the existing pairwise path. This is a security blocker for claiming complete group control; the group extension remains optional and its unresolved state is tracked in [`internal/audits/group-control.md`](../internal/audits/group-control.md).

MLS is not the MMP/1 group mechanism. A partitioned DTN can create concurrent membership changes without a delivery service to arbitrate commits. Any future MLS-compatible extension must specify conflict resolution under partition before reserving wire fields.
