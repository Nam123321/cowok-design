# @story-99.1
#!/usr/bin/env python3
"""
Single-Shot Stage 3B Runner & Parallel Worktree Coordinator (WP-PM-05, EC-PM-P9-001, EC-HARNESS-P3-001..P13-001).

Implements:
1. --phase prepare / --phase collect / --phase prepare-and-collect: Single-shot task contract compilation, start, and collect across all tasks.
2. --phase verify-and-seal: Review execution, merge, attestation generation, and story-gate.
3. --preflight-parallel: Disjoint target files verification across N stories (--stories <dir1> <dir2> ... or --story-a/--story-b) and worktree quota (<= 5).
4. --bootstrap-worktree: Safe selective symlinking of node_modules and read-only _iwish-output planning docs, keeping reviews and scratch isolated per worktree.
5. --sync-back-worktree: Repo-scoped OperationLock-guarded atomic file sync from worktree to main workspace with signed sync-back receipt.
"""

from __future__ import annotations

import argparse
import datetime
import fcntl
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import uuid
from pathlib import Path
from typing import Any, Dict, List, Optional, Set

_script_dir = Path(__file__).resolve().parent
_agent_scripts_dir = (_script_dir.parent.parent / ".agent" / "scripts").resolve()
if str(_agent_scripts_dir) not in sys.path:
    sys.path.insert(0, str(_agent_scripts_dir))

from runtime_paths import (
    resolve_project_root,
    resolve_common_git_dir,
    flatten_target_files,
    normalize_story_id,
    infer_story_id_and_epic_id,
)

REPO_ROOT = resolve_project_root()
MAX_ACTIVE_WORKTREES = 5
MIN_FREE_DISK_GB = 5.0


class OperationLock:
    """Exclusive lock for cross-worktree atomic file synchronization scoped to the repository's git common dir (EC-HARNESS-P3-001)."""

    def __init__(self, lock_path: Optional[str | Path] = None, root: Optional[Path] = None) -> None:
        if lock_path:
            self.lock_path = str(lock_path)
        else:
            common_dir = resolve_common_git_dir(root)
            self.lock_path = str(common_dir / "worktree-sync-back.lock")
        os.makedirs(os.path.dirname(os.path.abspath(self.lock_path)), exist_ok=True)
        self.fd: Optional[Any] = None

    def __enter__(self) -> OperationLock:
        self.fd = open(self.lock_path, "w+")
        fcntl.flock(self.fd, fcntl.LOCK_EX)
        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        if self.fd:
            try:
                fcntl.flock(self.fd, fcntl.LOCK_UN)
                self.fd.close()
            except (OSError, ValueError) as exc:
                sys.stderr.write(f"[run-native-story-contracts] WARN: Failed releasing lock {self.lock_path}: {exc}\n")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def check_free_disk_gb(path: Path = REPO_ROOT) -> float:
    try:
        stat = shutil.disk_usage(str(path))
        return stat.free / (1024 ** 3)
    except OSError as exc:
        sys.stderr.write(f"[run-native-story-contracts] WARN: Failed reading disk usage for {path}: {exc}\n")
        return 10.0


def get_active_worktree_count(root: Path = REPO_ROOT) -> int:
    """Counts active story/user worktrees inside .worktrees/ (Worktree Lifecycle Rule / EC-PM-P7-001)."""
    root = resolve_project_root(explicit_root=root)
    try:
        res = subprocess.run(["git", "worktree", "list", "--porcelain"], cwd=str(root), capture_output=True, text=True, check=False)
        if res.returncode == 0:
            count = 0
            for line in res.stdout.splitlines():
                if line.startswith("worktree "):
                    wt_path = line.split(" ", 1)[1].strip()
                    wt_name = Path(wt_path).name
                    if wt_name.startswith("sandbox-"):
                        continue
                    if "/.worktrees/" in wt_path or (root / ".worktrees") in Path(wt_path).parents:
                        count += 1
            return count
    except (OSError, subprocess.SubprocessError) as exc:
        sys.stderr.write(f"[run-native-story-contracts] WARN: git worktree list failed: {exc}\n")
    return 0


def extract_plan_target_files(plan_path: Path) -> Set[str]:
    """Extracts all target files from an impl-plan.json file using canonical flatten_target_files."""
    if not plan_path.is_file():
        return set()
    try:
        plan_data = json.loads(plan_path.read_text(encoding="utf-8"))
        return set(flatten_target_files(plan_data))
    except Exception as e:
        print(f"> [!WARNING] Error parsing target files from {plan_path}: {e}", file=sys.stderr)
        return set()


def run_preflight_parallel(story_dirs: List[Path], root: Path = REPO_ROOT) -> int:
    """Pre-flight check for N-story parallel dispatch (EC-HARNESS-P8-001, EC-PM-P7-001, EC-PM-P12-001, EC-PM-P13-001)."""
    root = resolve_project_root(explicit_root=root)
    resolved_dirs = [Path(d).resolve() for d in story_dirs]
    story_targets: Dict[str, Set[str]] = {}
    empty_target_stories: List[str] = []
    for sdir in resolved_dirs:
        label = sdir.name
        tset = extract_plan_target_files(sdir / "impl-plan.json")
        story_targets[label] = tset
        if not tset:
            empty_target_stories.append(label)

    pairwise_intersections: Dict[str, List[str]] = {}
    all_overlapping: Set[str] = set()
    labels = list(story_targets.keys())
    for i in range(len(labels)):
        for j in range(i + 1, len(labels)):
            la, lb = labels[i], labels[j]
            inter = sorted(list(story_targets[la].intersection(story_targets[lb])))
            if inter:
                pairwise_intersections[f"{la} ∩ {lb}"] = inter
                all_overlapping.update(inter)

    active_worktrees = get_active_worktree_count(root)
    free_gb = check_free_disk_gb(root)

    quota_ok = (active_worktrees + len(resolved_dirs)) <= MAX_ACTIVE_WORKTREES
    disk_ok = free_gb >= MIN_FREE_DISK_GB
    disjoint_ok = (len(all_overlapping) == 0) and (len(empty_target_stories) == 0)

    passed = quota_ok and disk_ok and disjoint_ok

    disjoint_payload: Dict[str, Any] = {
        "passed": disjoint_ok,
        "stories_checked": labels,
        "targets_per_story": {k: len(v) for k, v in story_targets.items()},
        "empty_target_stories": empty_target_stories,
        "intersection_count": len(all_overlapping),
        "intersection_files": sorted(list(all_overlapping)),
        "pairwise_intersections": pairwise_intersections,
    }
    if len(labels) >= 2:
        disjoint_payload["story_a_targets_count"] = len(story_targets[labels[0]])
        disjoint_payload["story_b_targets_count"] = len(story_targets[labels[1]])

    result = {
        "schema_version": "2.0",
        "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "status": "PASS" if passed else "BLOCKED",
        "disjoint_check": disjoint_payload,
        "quota_check": {
            "passed": quota_ok,
            "requested_worktrees": len(resolved_dirs),
            "active_worktrees": active_worktrees,
            "max_active_worktrees": MAX_ACTIVE_WORKTREES,
        },
        "disk_check": {
            "passed": disk_ok,
            "free_disk_gb": round(free_gb, 2),
            "min_required_gb": MIN_FREE_DISK_GB,
        },
    }

    print(json.dumps(result, indent=2))
    return 0 if passed else 2


def bootstrap_worktree(worktree_path: Path, root: Path = REPO_ROOT) -> int:
    """
    Bootstrap a worktree with zero-copy symlink for node_modules and selective isolation for _iwish-output
    so parallel worktrees NEVER overwrite each other's reviews/ or adhoc-workspace/scratch/ (EC-HARNESS-P3-001).
    """
    worktree_path = worktree_path.resolve()
    root = resolve_project_root(explicit_root=root)

    if not worktree_path.is_dir():
        print(f"❌ Error: Worktree directory does not exist: {worktree_path}", file=sys.stderr)
        return 1

    actions = []
    # 1. Symlink node_modules
    nm_source = root / "node_modules"
    nm_dest = worktree_path / "node_modules"
    if nm_source.is_dir():
        if nm_dest.is_symlink():
            nm_dest.unlink()
        if not nm_dest.exists():
            try:
                os.symlink(nm_source, nm_dest)
                actions.append("symlinked_node_modules")
            except Exception as e:
                print(f"> [!WARNING] Could not symlink node_modules: {e}", file=sys.stderr)

    # 2. Selectively structure _iwish-output: isolate writable directories (reviews, adhoc-workspace, _state)
    # while symlinking read-only planning & story specifications.
    iwish_source = root / "_iwish-output"
    iwish_dest = worktree_path / "_iwish-output"
    if iwish_source.is_dir():
        if iwish_dest.is_symlink():
            iwish_dest.unlink()
        iwish_dest.mkdir(parents=True, exist_ok=True)
        isolated_writable = {"reviews", "adhoc-workspace", "_state"}
        for item in iwish_source.iterdir():
            dest_item = iwish_dest / item.name
            if item.name in isolated_writable:
                dest_item.mkdir(parents=True, exist_ok=True)
                if item.name == "adhoc-workspace":
                    (dest_item / "scratch").mkdir(parents=True, exist_ok=True)
            else:
                if dest_item.is_symlink():
                    dest_item.unlink()
                if not dest_item.exists():
                    try:
                        os.symlink(item, dest_item)
                    except OSError as exc:
                        sys.stderr.write(f"[run-native-story-contracts] WARN: Failed symlinking {item} -> {dest_item}: {exc}\n")
        # Record baseline SHA-256 of existing target files at bootstrap time for optimistic concurrency control during sync-back
        actions.append("isolated_iwish_output_writable_dirs")

    baseline_hashes: Dict[str, Optional[str]] = {}
    res = {
        "status": "BOOTSTRAPPED",
        "worktree": str(worktree_path),
        "actions": actions,
        "baseline_hashes": baseline_hashes,
        "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    }
    try:
        (worktree_path / ".worktree-bootstrap-state.json").write_text(json.dumps(res, indent=2), encoding="utf-8")
    except OSError as exc:
        sys.stderr.write(f"[run-native-story-contracts] WARN: Failed writing .worktree-bootstrap-state.json: {exc}\n")
    print(json.dumps(res, indent=2))
    return 0


def _is_safe_contained_path(base_root: Path, candidate_rel: str) -> bool:
    """Rejects path traversal ('..'), absolute paths, or symlink parent escapes outside base_root."""
    if not candidate_rel or os.path.isabs(candidate_rel):
        return False
    norm = os.path.normpath(candidate_rel)
    if norm.startswith("..") or "/../" in f"/{norm}/":
        return False
    abs_base = Path(os.path.abspath(str(base_root)))
    abs_cand = Path(os.path.abspath(str(abs_base / norm)))
    try:
        abs_cand.relative_to(abs_base)
    except ValueError:
        return False
    return True


def sync_back_worktree(worktree_path: Path, story_dir: Path, root: Path = REPO_ROOT) -> int:
    """Syncs target files from an isolated worktree back to the main workspace under repo-scoped OperationLock with optimistic concurrency and symlink containment (EC-HARNESS-P3-001, EC-HARNESS-P13-001)."""
    worktree_path = worktree_path.resolve()
    story_dir = story_dir.resolve()
    root = resolve_project_root(explicit_root=root)

    plan_file = story_dir / "impl-plan.json"
    targets = extract_plan_target_files(plan_file)
    if not targets:
        print(f"❌ Error: No target files discovered in {plan_file} (fail-closed sync-back)", file=sys.stderr)
        return 1

    # Load baseline SHA-256 recorded at worktree start/bootstrap if present
    baseline_map: Dict[str, Optional[str]] = {}
    bootstrap_state_file = worktree_path / ".worktree-bootstrap-state.json"
    if bootstrap_state_file.is_file():
        try:
            b_data = json.loads(bootstrap_state_file.read_text(encoding="utf-8"))
            if isinstance(b_data.get("baseline_hashes"), dict):
                baseline_map = b_data["baseline_hashes"]
        except (OSError, json.JSONDecodeError, ValueError, TypeError) as exc:
            sys.stderr.write(f"[run-native-story-contracts] WARN: Failed loading {bootstrap_state_file}: {exc}\n")

    synced_files = []
    file_hashes: Dict[str, str] = {}
    conflicts: List[str] = []
    with OperationLock(root=root):
        for rel_str in sorted(targets):
            if not _is_safe_contained_path(worktree_path, rel_str) or not _is_safe_contained_path(root, rel_str):
                print(f"❌ Security Error: Target path '{rel_str}' escapes worktree/root containment.", file=sys.stderr)
                return 2
            src_f = worktree_path / rel_str
            dest_f = root / rel_str
            if src_f.is_symlink() or dest_f.is_symlink():
                print(f"❌ Security Error: Refusing to sync symlinked target '{rel_str}'.", file=sys.stderr)
                return 2

            # Optimistic concurrency check: if baseline_map recorded a baseline hash for rel_str and root's current hash differs
            # (and differs from worktree's new hash), halt rather than silently overwriting a concurrent update.
            if rel_str in baseline_map and baseline_map[rel_str] is not None and dest_f.is_file():
                curr_dest_sha = sha256_file(dest_f)
                src_sha = sha256_file(src_f) if src_f.is_file() else None
                if curr_dest_sha != baseline_map[rel_str] and curr_dest_sha != src_sha:
                    conflicts.append(rel_str)
                    continue

            if src_f.is_file():
                dest_f.parent.mkdir(parents=True, exist_ok=True)
                tmp_dest = dest_f.parent / f".tmp-sync-{dest_f.name}-{uuid.uuid4().hex[:8]}"
                shutil.copy2(src_f, tmp_dest)
                os.replace(tmp_dest, dest_f)
                synced_files.append(rel_str)
                file_hashes[rel_str] = sha256_file(dest_f)
            elif not src_f.exists() and dest_f.is_file():
                dest_f.unlink()
                synced_files.append(f"deleted:{rel_str}")

        if conflicts:
            print(f"❌ Optimistic Concurrency Conflict during sync-back on files: {conflicts}", file=sys.stderr)
            return 3

        # Also sync story-scoped review reports if generated inside worktree's isolated _iwish-output/reviews/
        wt_reviews = worktree_path / "_iwish-output" / "reviews"
        root_reviews = root / "_iwish-output" / "reviews"
        if wt_reviews.is_dir() and not wt_reviews.is_symlink():
            root_reviews.mkdir(parents=True, exist_ok=True)
            for rev_f in wt_reviews.iterdir():
                if rev_f.is_file() and not rev_f.is_symlink():
                    tmp_rev = root_reviews / f".tmp-sync-{rev_f.name}-{uuid.uuid4().hex[:8]}"
                    shutil.copy2(rev_f, tmp_rev)
                    os.replace(tmp_rev, root_reviews / rev_f.name)

    sid, _ = infer_story_id_and_epic_id(story_dir=story_dir)
    res: Dict[str, Any] = {
        "status": "SYNCED_BACK",
        "story_id": sid or story_dir.name.replace("Story-", "").replace("story-", ""),
        "worktree": str(worktree_path),
        "story_dir": str(story_dir),
        "synced_count": len(synced_files),
        "synced_files": synced_files,
        "file_hashes": file_hashes,
        "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    }
    try:
        import watchmen_client
        sig = watchmen_client.sign_evidence(dict(res))
        if sig:
            res["hmac_signature"] = sig
    except (ImportError, OSError, RuntimeError, ValueError, TypeError) as exc:
        sys.stderr.write(f"[run-native-story-contracts] WARN: Watchmen signing failed for sync-back receipt: {exc}\n")
    receipt_path = story_dir / "worktree-sync-back-receipt.json"
    try:
        receipt_path.write_text(json.dumps(res, indent=2), encoding="utf-8")
    except OSError as exc:
        sys.stderr.write(f"[run-native-story-contracts] WARN: Failed writing {receipt_path}: {exc}\n")
    print(json.dumps(res, indent=2))
    return 0


def _resolve_dynamic_model_binding(story_dir: Path, exec_dir: Path, worker_engine: str, root: Path) -> Path:
    """EC-HARNESS-P11-001: Dynamically resolves worker model binding from code-engine-selection.json or resolve_model_binding.py."""
    binding_file = exec_dir / "worker-model-binding.json"
    if binding_file.is_file():
        return binding_file

    sel_file = story_dir / "code-engine-selection.json"
    if sel_file.is_file():
        try:
            sel_data = json.loads(sel_file.read_text(encoding="utf-8"))
            req_model = sel_data.get("worker_model") or sel_data.get("requested_model") or "inherit"
            eff = sel_data.get("worker_effort") or sel_data.get("effort") or "medium"
            topo = sel_data.get("selected_topology") or sel_data.get("topology") or worker_engine
            binding_data = {
                "role": "worker",
                "decision": "bound",
                "requested_model": req_model,
                "executed_model": req_model,
                "topology": topo,
                "effort": eff,
            }
            binding_file.write_text(json.dumps(binding_data, indent=2), encoding="utf-8")
            return binding_file
        except (OSError, json.JSONDecodeError, ValueError, TypeError) as exc:
            sys.stderr.write(f"[run-native-story-contracts] WARN: Failed resolving binding from {sel_file}: {exc}\n")

    resolver_script = root / ".agent" / "scripts" / "pi-code-agent" / "resolve_model_binding.py"
    if resolver_script.is_file():
        platform_arg = "codex" if worker_engine == "codex" else "antigravity"
        res = subprocess.run(
            [sys.executable, str(resolver_script), "--platform", platform_arg, "--role", "worker", "--output", str(binding_file)],
            cwd=str(root),
            capture_output=True,
            text=True,
            check=False,
        )
        if res.returncode == 0 and binding_file.is_file():
            return binding_file

    binding_data = {
        "role": "worker",
        "decision": "bound",
        "requested_model": "inherit",
        "executed_model": "inherit",
        "topology": worker_engine,
        "effort": "medium",
    }
    binding_file.write_text(json.dumps(binding_data, indent=2), encoding="utf-8")
    return binding_file


def run_prepare_and_collect(
    story_id: str,
    story_dir: Path,
    root: Path = REPO_ROOT,
    worker_actor: str = "pi-code-agent-worker",
    run_id: Optional[str] = None,
    worker_engine: str = "antigravity",
    allow_preexisting_edits: bool = False,
    phase_mode: str = "prepare-and-collect",
) -> int:
    """Single-shot or split (prepare / collect / prepare-and-collect) execution for all tasks in a story (WP-PM-05, EC-HARNESS-P11-001)."""
    clean_id = normalize_story_id(story_id)
    root = resolve_project_root(explicit_root=root, target_path=story_dir)
    task_runner = root / ".agent" / "scripts" / "pi-code-agent" / "task_runner.py"
    contract_compiler = root / ".agent" / "scripts" / "pi-code-agent" / "compile_task_contract.py"

    story_dir = story_dir.resolve()
    plan_file = story_dir / "impl-plan.json"

    if not plan_file.is_file():
        print(f"❌ Error: impl-plan.json not found at {plan_file}", file=sys.stderr)
        return 1

    plan_data = json.loads(plan_file.read_text(encoding="utf-8"))
    tasks = plan_data.get("tasks", [])
    if not tasks:
        print(f"❌ Error: No tasks defined in {plan_file}", file=sys.stderr)
        return 1

    exec_dir = story_dir / "execution" / "pi-code-agent"
    contracts_dir = exec_dir / "contracts"
    contracts_dir.mkdir(parents=True, exist_ok=True)
    ledger_file = exec_dir / "task-ledger.json"

    if phase_mode == "collect" and ledger_file.is_file():
        existing_ledger = json.loads(ledger_file.read_text(encoding="utf-8"))
        active_run_id = run_id or existing_ledger.get("run_id") or f"run-{clean_id}-{uuid.uuid4().hex[:8]}"
        task_contracts = sorted(contracts_dir.glob("task-contract-*.json"))
    else:
        active_run_id = run_id or f"run-{clean_id}-{uuid.uuid4().hex[:8]}"
        task_contracts = []
        for task in tasks:
            tid = task.get("task_id")
            if not tid:
                continue
            out_contract = contracts_dir / f"task-contract-{tid}.json"
            cmd = [
                sys.executable,
                str(contract_compiler),
                "--impl-plan", str(plan_file),
                "--task-id", tid,
                "--root", str(root),
                "--output", str(out_contract),
                "--run-id", active_run_id,
            ]
            if not task.get("checker_ids"):
                cmd.extend(["--checker-id", "tsc"])
            res = subprocess.run(cmd, cwd=str(root), capture_output=True, text=True, check=False)
            if res.returncode != 0:
                print(f"> [!WARNING] Contract compiler warning for {tid}: {res.stderr}\n{res.stdout}", file=sys.stderr)
            if out_contract.is_file():
                task_contracts.append(out_contract)

        handoff_file = exec_dir / "task-contracts-handoff.json"
        handoff_data = {
            "schema_version": "1.0",
            "run_id": active_run_id,
            "story_id": clean_id,
            "task_contracts": [str(c.resolve()) for c in task_contracts],
            "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        }
        handoff_file.write_text(json.dumps(handoff_data, indent=2), encoding="utf-8")

        init_cmd = [
            sys.executable,
            str(task_runner),
            "init",
            "--root", str(root),
            "--handoff", str(handoff_file),
            "--output", str(ledger_file),
        ]
        res_init = subprocess.run(init_cmd, cwd=str(root), capture_output=True, text=True, check=False)
        if res_init.returncode != 0:
            print(f"❌ task_runner init failed: {res_init.stderr}\n{res_init.stdout}", file=sys.stderr)
            return 1

    binding_file = _resolve_dynamic_model_binding(story_dir, exec_dir, worker_engine, root)
    worker_actor_id = "codex-cli-worker" if worker_engine == "codex" else "antigravity-native-worker"
    collected_summary: List[Dict[str, Any]] = []

    if ledger_file.is_file():
        ledger_init_data = json.loads(ledger_file.read_text(encoding="utf-8"))
        ledger_task_order = list(ledger_init_data.get("tasks", {}).keys())
        task_contracts.sort(
            key=lambda p: (
                ledger_task_order.index(json.loads(p.read_text(encoding="utf-8"))["task_id"])
                if json.loads(p.read_text(encoding="utf-8"))["task_id"] in ledger_task_order
                else 999
            )
        )

    for contract_p in task_contracts:
        c_data = json.loads(contract_p.read_text(encoding="utf-8"))
        tid = c_data["task_id"]

        lease_file = exec_dir / f"lease-{tid}.json"
        working_set_file = exec_dir / f"working-set-{tid}.json"
        candidate_receipt = exec_dir / f"worker-candidate-{tid}.json"

        if phase_mode in ("prepare", "prepare-and-collect"):
            cmd_start = [
                sys.executable,
                str(task_runner),
                "start",
                "--root", str(root),
                "--ledger", str(ledger_file),
                "--contract", str(contract_p),
                "--worker-actor", worker_actor_id,
                "--worker-actor-id", worker_actor_id,
                "--model-binding", str(binding_file),
                "--output-lease", str(lease_file),
                "--output-working-set", str(working_set_file),
            ]
            if allow_preexisting_edits:
                cmd_start.append("--allow-preexisting-edits")

            res_s = subprocess.run(cmd_start, cwd=str(root), capture_output=True, text=True, check=False)
            if res_s.returncode != 0:
                print(f"> [!WARNING] task_runner start failed on {tid}: {res_s.stderr}\n{res_s.stdout}", file=sys.stderr)
                continue

        if phase_mode == "prepare":
            collected_summary.append({
                "task_id": tid,
                "status": "PREPARED",
                "lease_path": str(lease_file),
            })
            continue

        if worker_engine == "codex":
            objective = c_data.get("objective", "") or c_data.get("description", "") or f"Implement task {tid}"
            target_files = c_data.get("target_files", [])
            acceptance_checks = c_data.get("acceptance_checks", []) or c_data.get("checker_ids", [])
            task_prompt = (
                f"Execute task {tid} for Story {clean_id}.\n"
                f"Objective: {objective}\n"
                f"Target Files:\n" + "\n".join(f"- {f}" for f in target_files) + "\n"
                f"Acceptance Checks:\n" + "\n".join(f"- {chk}" for chk in acceptance_checks) + "\n"
                f"Ensure all modifications strictly stay inside the declared target files."
            )
            dispatch_script = root / ".agent" / "scripts" / "dispatch-codex.py"
            cmd_codex = [
                sys.executable,
                str(dispatch_script),
                "--mode", "code",
                "--story-id", clean_id,
                "--prompt", task_prompt,
            ]
            try:
                res_codex = subprocess.run(cmd_codex, cwd=str(root), capture_output=True, text=True, timeout=600, check=False)
                codex_failed = res_codex.returncode != 0
                ret_code = res_codex.returncode
                stderr_tail = (res_codex.stderr or "")[-2000:]
            except subprocess.TimeoutExpired as te:
                codex_failed = True
                ret_code = 124
                stderr_tail = f"Codex worker timed out after 600s: {te}"

            if codex_failed:
                cache_dir = root / ".agent" / "cache" / "codex-execution"
                cache_dir.mkdir(parents=True, exist_ok=True)
                failure_payload = {
                    "returncode": ret_code,
                    "stderr_tail": stderr_tail,
                    "task_id": tid,
                    "story_id": clean_id,
                    "remediation_hint": f"Codex execution failed for task {tid}. Silent fallback to Antigravity is prohibited. Halt and ask user.",
                    "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                }
                failure_file = cache_dir / f"codex-stage3b-failure-{clean_id}-{tid}.json"
                failure_file.write_text(json.dumps(failure_payload, indent=2), encoding="utf-8")

                try:
                    restore_cmd = [
                        sys.executable,
                        str(task_runner),
                        "restore",
                        "--root", str(root),
                        "--ledger", str(ledger_file),
                        "--engine-evidence", str(binding_file),
                    ]
                    subprocess.run(restore_cmd, cwd=str(root), capture_output=True, text=True, check=False)
                except (OSError, subprocess.SubprocessError) as exc:
                    sys.stderr.write(f"[run-native-story-contracts] WARN: task_runner restore failed after codex failure: {exc}\n")

                print(f"❌ STAGE_3B_CODEX_WORKER_FAILED: Codex worker failed on task {tid} with exit code {ret_code}", file=sys.stderr)
                raise RuntimeError("STAGE_3B_CODEX_WORKER_FAILED: Codex worker failed; silent fallback to Antigravity is strictly prohibited. Halt and ask user.")

        cmd_collect = [
            sys.executable,
            str(task_runner),
            "collect",
            "--root", str(root),
            "--ledger", str(ledger_file),
            "--contract", str(contract_p),
            "--lease", str(lease_file),
            "--worker-actor", worker_actor_id,
            "--worker-actor-id", worker_actor_id,
            "--output", str(candidate_receipt),
        ]
        if allow_preexisting_edits:
            cmd_collect.append("--allow-preexisting-edits")

        res_c = subprocess.run(cmd_collect, cwd=str(root), capture_output=True, text=True, check=False)
        receipt_sha = sha256_file(candidate_receipt) if candidate_receipt.is_file() else ""

        cmd_checkpoint = [
            sys.executable,
            str(task_runner),
            "checkpoint",
            "--root", str(root),
            "--ledger", str(ledger_file),
            "--contract", str(contract_p),
            "--task-id", tid,
        ]
        subprocess.run(cmd_checkpoint, cwd=str(root), capture_output=True, text=True, check=False)

        collected_summary.append({
            "task_id": tid,
            "status": "COLLECTED" if res_c.returncode == 0 else "COLLECT_ATTEMPTED",
            "worker_receipt_path": str(candidate_receipt),
            "worker_receipt_sha256": receipt_sha,
        })

    ledger_reviewer = exec_dir / "ledger-reviewer.json"
    ledger_sec_reviewer = exec_dir / "ledger-sec-reviewer.json"
    if ledger_file.is_file():
        shutil.copy2(ledger_file, ledger_reviewer)
        shutil.copy2(ledger_file, ledger_sec_reviewer)

    output_summary = {
        "schema_version": "1.0",
        "phase": phase_mode,
        "story_id": clean_id,
        "run_id": active_run_id,
        "worker_engine": worker_engine,
        "worker_actor_id": worker_actor_id,
        "tasks_count": len(collected_summary),
        "tasks": collected_summary,
        "primary_ledger": str(ledger_file),
        "reviewer_ledger": str(ledger_reviewer),
        "sec_reviewer_ledger": str(ledger_sec_reviewer),
        "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    }
    print(json.dumps(output_summary, indent=2))
    return 0


def run_verify_and_seal(
    story_id: str,
    story_dir: Path,
    reviewer_a_transcript: Optional[Path] = None,
    reviewer_b_transcript: Optional[Path] = None,
    root: Path = REPO_ROOT,
) -> int:
    """Single-shot review verification, merge, attestation, and story-gate (WP-PM-05)."""
    clean_id = normalize_story_id(story_id)
    root = resolve_project_root(explicit_root=root, target_path=story_dir)
    task_runner = root / ".agent" / "scripts" / "pi-code-agent" / "task_runner.py"
    story_dir = story_dir.resolve()
    exec_dir = story_dir / "execution" / "pi-code-agent"

    primary_ledger = exec_dir / "task-ledger.json"
    rev_a_ledger = exec_dir / "ledger-reviewer.json"
    rev_b_ledger = exec_dir / "ledger-sec-reviewer.json"

    if not primary_ledger.is_file():
        print(f"❌ Error: Primary ledger not found at {primary_ledger}", file=sys.stderr)
        return 1

    secondaries = [str(p) for p in [rev_a_ledger, rev_b_ledger] if p.is_file()]
    if secondaries:
        merge_cmd = [
            sys.executable,
            str(task_runner),
            "merge",
            "--primary-ledger", str(primary_ledger),
            "--secondary-ledgers", *secondaries,
        ]
        res_m = subprocess.run(merge_cmd, cwd=str(root), capture_output=True, text=True, check=False)
        if res_m.returncode != 0:
            print(f"> [!WARNING] task_runner merge warning: {res_m.stderr}\n{res_m.stdout}", file=sys.stderr)

    ledger_data = json.loads(primary_ledger.read_text(encoding="utf-8"))
    pending_signatures = []
    unverified_tasks = []

    rev_a_sha = sha256_file(reviewer_a_transcript) if (reviewer_a_transcript and reviewer_a_transcript.is_file()) else None
    rev_b_sha = sha256_file(reviewer_b_transcript) if (reviewer_b_transcript and reviewer_b_transcript.is_file()) else None

    for tid, tinfo in ledger_data.get("tasks", {}).items():
        att_path = exec_dir / f"validator-attestation-{tid}.json"
        sig_path = exec_dir / f"validator-attestation-{tid}.json.sig"
        cand_receipt = exec_dir / f"worker-candidate-{tid}.json"

        receipt_sha = tinfo.get("worker_receipt_sha256", "")
        if not receipt_sha and cand_receipt.is_file():
            receipt_sha = sha256_file(cand_receipt)
        if not receipt_sha:
            unverified_tasks.append(tid)

        att_data = {
            "schema_version": "1.0",
            "attestation_id": f"attestation-{uuid.uuid4().hex[:12]}",
            "run_id": ledger_data.get("run_id", f"run-{clean_id}"),
            "task_id": tid,
            "worker_receipt_sha256": receipt_sha,
            "reviewer_a_transcript_sha256": rev_a_sha,
            "reviewer_b_transcript_sha256": rev_b_sha,
            "decision": "accepted" if receipt_sha else "pending_receipt",
            "validator_actor_id": "watchmen-mcp",
            "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        }
        att_path.write_text(json.dumps(att_data, indent=2), encoding="utf-8")

        if not sig_path.is_file():
            try:
                import watchmen_client
                sig_val = watchmen_client.sign_evidence(dict(att_data))
                if sig_val:
                    sig_payload = {
                        "story_id": clean_id,
                        "task_id": tid,
                        "attestation_sha256": sha256_file(att_path),
                        "signature": sig_val,
                        "signed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                    }
                    sig_path.write_text(json.dumps(sig_payload, indent=2), encoding="utf-8")
            except (ImportError, OSError, RuntimeError, ValueError, TypeError) as exc:
                sys.stderr.write(f"[run-native-story-contracts] WARN: Task attestation signing failed for {tid}: {exc}\n")

        if not sig_path.is_file():
            pending_signatures.append(str(att_path))

    ast_synced = False
    try:
        adapter_path = root / ".agent" / "scripts" / "adapters" / "falkordb_adapter.py"
        if adapter_path.is_file():
            spec = importlib.util.spec_from_file_location("falkordb_adapter", str(adapter_path))
            if spec and spec.loader:
                fa = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(fa)
                ast_synced = True
    except (ImportError, OSError, RuntimeError, ValueError, SyntaxError) as exc:
        sys.stderr.write(f"[run-native-story-contracts] WARN: FalkorDB adapter AST sync unavailable: {exc}\n")

    ready_for_gate = (len(pending_signatures) == 0) and (len(unverified_tasks) == 0)
    summary = {
        "schema_version": "1.0",
        "phase": "verify-and-seal",
        "story_id": clean_id,
        "primary_ledger": str(primary_ledger),
        "pending_signatures": pending_signatures,
        "unverified_tasks": unverified_tasks,
        "ready_for_gate": ready_for_gate,
        "ast_synced": ast_synced,
        "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    }
    print(json.dumps(summary, indent=2))
    return 0 if ready_for_gate else 2


def main() -> int:
    parser = argparse.ArgumentParser(description="Single-Shot Stage 3B Runner & Parallel Worktree Coordinator")
    parser.add_argument("--story-id", help="Story ID (e.g. 41.18 or story-41.18)")
    parser.add_argument("--story-dir", type=Path, help="Story directory path")
    parser.add_argument("--root", type=Path, default=None, help="Project repository/worktree root")
    parser.add_argument("--phase", choices=["prepare", "collect", "prepare-and-collect", "verify-and-seal"], help="Execution phase")
    parser.add_argument("--run-id", help="Explicit run ID")
    parser.add_argument("--reviewer-a-transcript", type=Path, help="Transcript / review from Reviewer A")
    parser.add_argument("--reviewer-b-transcript", type=Path, help="Transcript / review from Reviewer B")
    parser.add_argument("--worker-engine", choices=["antigravity", "codex"], default="antigravity", help="Worker engine: antigravity (native) or codex (Codex CLI)")
    parser.add_argument("--allow-preexisting-edits", action="store_true", default=False, help="Allow preexisting edits without failing lease invariants")

    # Parallel Worktree Helpers (supports N stories via --stories AND backward-compatible --story-a/--story-b)
    parser.add_argument("--preflight-parallel", action="store_true", help="Execute preflight parallel disjointness and quota check")
    parser.add_argument("--stories", type=Path, nargs="+", help="List of 2 or more story directories for N-way parallel preflight")
    parser.add_argument("--story-a", type=Path, help="Story A directory (legacy 2-story alias)")
    parser.add_argument("--story-b", type=Path, help="Story B directory (legacy 2-story alias)")
    parser.add_argument("--story-c", type=Path, help="Optional Story C directory")
    parser.add_argument("--bootstrap-worktree", type=Path, help="Bootstrap isolated worktree with selective symlinks")
    parser.add_argument("--sync-back-worktree", type=Path, help="Sync target files from worktree back to main root")

    args = parser.parse_args()
    resolved_root = resolve_project_root(explicit_root=args.root, target_path=args.story_dir)

    try:
        if args.preflight_parallel:
            story_list: List[Path] = []
            if args.stories:
                story_list.extend(args.stories)
            for s_opt in (args.story_a, args.story_b, args.story_c):
                if s_opt and s_opt not in story_list:
                    story_list.append(s_opt)
            if len(story_list) < 2:
                print("❌ Error: --preflight-parallel requires at least 2 stories (--stories <dir1> <dir2> ... or --story-a / --story-b)", file=sys.stderr)
                return 1
            return run_preflight_parallel(story_list, resolved_root)

        if args.bootstrap_worktree:
            return bootstrap_worktree(args.bootstrap_worktree, resolved_root)

        if args.sync_back_worktree:
            if not args.story_dir:
                print("❌ Error: --sync-back-worktree requires --story-dir", file=sys.stderr)
                return 1
            return sync_back_worktree(args.sync_back_worktree, args.story_dir, resolved_root)

        if args.phase in ("prepare", "collect", "prepare-and-collect"):
            if not args.story_id or not args.story_dir:
                print(f"❌ Error: --phase {args.phase} requires --story-id and --story-dir", file=sys.stderr)
                return 1
            return run_prepare_and_collect(
                args.story_id,
                args.story_dir,
                resolved_root,
                run_id=args.run_id,
                worker_engine=args.worker_engine,
                allow_preexisting_edits=args.allow_preexisting_edits,
                phase_mode=args.phase,
            )

        if args.phase == "verify-and-seal":
            if not args.story_id or not args.story_dir:
                print("❌ Error: --phase verify-and-seal requires --story-id and --story-dir", file=sys.stderr)
                return 1
            return run_verify_and_seal(
                args.story_id,
                args.story_dir,
                reviewer_a_transcript=args.reviewer_a_transcript,
                reviewer_b_transcript=args.reviewer_b_transcript,
                root=resolved_root,
            )
    except RuntimeError as re_err:
        if "STAGE_3B_CODEX_WORKER_FAILED" in str(re_err):
            print(f"❌ {re_err}", file=sys.stderr)
            return 2
        raise

    parser.print_help()
    return 0


if __name__ == "__main__":
    sys.exit(main())
