# Host hardware lock

This action serializes GPU/browser hardware jobs across repositories that run
under the same OS account on one machine. It uses an atomically created
directory under the OS temporary directory, records the owning GitHub run, and
removes only a lock whose random token still matches the acquiring job.

The post action releases the lock after success, failure, or cancellation.
Abandoned locks become recoverable after `stale_seconds`; the default four-hour
lease is longer than the triplet workflows' maximum post-acquisition runtime.
Set `WGPU_HARDWARE_LOCK_ROOT` when multiple runner services on one host do not
share an OS temporary directory.

Scry and Weld pin this action by full Graft commit. A behavior change therefore
requires a new Graft commit and explicit pin updates in both consumers.
