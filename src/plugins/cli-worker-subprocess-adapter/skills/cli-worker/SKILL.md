---
name: cli-worker
version: 1.0.0
risk_tier: 2
description: Isolated CLI worker subprocess adapter with -PGID reaping and shadow buffering.
---

# CLI Worker Subprocess Adapter

Run per-node CLI codegen tasks safely inside the daemon's keyed execution pool:

1. Acquire a slot (`running + paused <= 4`) and receive `(jobId, fencingToken)` from `POST /api/projects/:id/flow/jobs/enqueue`.
2. Spawn the worker in its own process group (`setpgid(0, 0)`) writing speculative mutations exclusively under `.cowok-design/shadow/<project_id>/<job_id>/`.
3. Attach `expectedFencingToken` to every state/event mutation so stale workers fail closed with `409 STALE_FENCING_TOKEN`.
4. On `steer` or `cancel`, discard the unpromoted shadow directory and reap `-PGID` (`SIGTERM -> 250ms -> SIGKILL`) before bumping `fencingToken`.
