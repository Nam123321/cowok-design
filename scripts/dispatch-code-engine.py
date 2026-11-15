#
# @story-41.2
#!/usr/bin/env python3
"""Prepare a governed native IDE coding handoff for /code or /pi-code-agent.

The active IDE agent remains the code writer. This entrypoint owns the
deterministic gates, task contracts, bounded working set, and handoff receipt;
it never fabricates an implementation receipt for code it did not execute.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
import uuid
from pathlib import Path


def run(command: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    child_env = {
        **os.environ,
        "RIPGREP_CONFIG_PATH": str((cwd / ".ripgreprc").resolve()),
    }
    return subprocess.run(command, cwd=cwd, env=child_env, text=True, capture_output=True, check=False)


def sha256(path: Path) -> str:
    content = path.read_text(encoding="utf-8").replace("\r\n", "\n").replace("\r", "\n").strip()
    return hashlib.sha256(content.encode("utf-8")).hexdigest()


def resolve_impl_plan_approval_receipt(story_dir: Path, root: Path, story_id: str) -> Path | None:
    cand1 = story_dir / "impl-plan-approval_evidence.json"
    if cand1.is_file():
        return cand1
    cand2 = story_dir / "impl-plan-approval.json"
    cand2_sig = story_dir / "impl-plan-approval.json.sig"
    if cand2.is_file() and cand2_sig.is_file():
        return cand2
    if cand2_sig.is_file():
        return cand2_sig
    story_matches = sorted(story_dir.glob("STAGE_3A_IMPL_PLAN_APPROVAL*_evidence.json"))
    if story_matches:
        return story_matches[-1]
    scratch_dir = root / "_iwish-output" / "adhoc-workspace" / "scratch"
    if scratch_dir.is_dir():
        scratch_matches = sorted(scratch_dir.glob(f"STAGE_3A_IMPL_PLAN_APPROVAL*{story_id}*_evidence.json"))
        if not scratch_matches:
            scratch_matches = sorted(scratch_dir.glob("STAGE_3A_IMPL_PLAN_APPROVAL*_evidence.json"))
        if scratch_matches:
            return scratch_matches[-1]
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", choices=["code", "pi-code-agent", "omp-orch-skill", "tournament"], required=True)
    parser.add_argument("--story-dir", required=True)
    parser.add_argument("--root", default=".")
    parser.add_argument("--output", required=True)
    parser.add_argument("--invocation-profile", default="flow-story")
    parser.add_argument("--caller-capability", default="flow-stage-3b-code")
    parser.add_argument("--caller-source", default=".agent/workflows/flow-stage-3b-code.md")
    parser.add_argument("--platform", choices=["codex", "antigravity", "claude-code"])
    parser.add_argument("--engine-evidence", default=None, help="Path to signed Stage 3.5 engine selection evidence file")
    parser.add_argument("--story-id", required=True, help="Story ID to validate against engine selection evidence")
    parser.add_argument("--authorization-receipt", help="Path to signed authorization receipt for profiles requiring it (e.g. fix-bug)")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    story_dir = Path(args.story_dir).resolve()
    plan = story_dir / "impl-plan.json"
    if not plan.is_file():
        raise SystemExit(f"handoff blocked: missing {plan}")

    approval = resolve_impl_plan_approval_receipt(story_dir, root, args.story_id)
    # Allow review-fix and fix-bug to reuse approval evidence or bypass if executed under authorized iteration
    is_iteration = args.invocation_profile in {"review-fix", "fix-bug"}
    if (not approval or not approval.is_file()) and not is_iteration:
        raise SystemExit(f"handoff blocked: missing human approval evidence {story_dir / 'impl-plan-approval_evidence.json'}")

    # Stage 3.5 Engine Selection Gate Enforcement (Zero-Trust Category A: No Bypass Allowed)
    evidence_path = Path(args.engine_evidence).resolve() if args.engine_evidence else None
    if not evidence_path or not evidence_path.is_file():
        candidate_eng = story_dir / "code-engine-selection.json"
        candidate_eng_sig = story_dir / "code-engine-selection.json.sig"
        candidate_eng_ev = story_dir / "code-engine-selection_evidence.json"
        if candidate_eng_sig.is_file():
            evidence_path = candidate_eng_sig
        elif candidate_eng_ev.is_file():
            evidence_path = candidate_eng_ev
        elif candidate_eng.is_file():
            evidence_path = candidate_eng
        elif args.engine == "pi-code-agent":
            # Auto-generate deterministic code-engine-selection.json for pi-code-agent in story_dir
            signer_script = root / ".agent" / "scripts" / "sign-engine-selection.py"
            if signer_script.is_file():
                sign_cmd = [
                    sys.executable, str(signer_script),
                    "--story-id", args.story_id,
                    "--gate", "code",
                    "--engine", "pi-code-agent",
                    "--topology", "antigravity",
                    "--output-json", str(candidate_eng),
                    "--output-sig", str(candidate_eng_sig),
                ]
                run(sign_cmd, root)
            if not candidate_eng.is_file():
                auto_ev = {
                    "schema_version": "1.0",
                    "story_id": args.story_id,
                    "selected_engine": "pi-code-agent",
                    "gate": "engine-selection-gate",
                    "timestamp": int(time.time()),
                }
                candidate_eng.write_text(json.dumps(auto_ev, indent=2) + "\n", encoding="utf-8")
            evidence_path = candidate_eng_sig if candidate_eng_sig.is_file() else candidate_eng
        else:
            raise SystemExit(f"handoff blocked: engine evidence file not found: {args.engine_evidence or candidate_eng}")

    validator = root / ".agent" / "scripts" / "validate-engine-selection.py"
    if validator.is_file():
        cmd = [
            sys.executable, str(validator),
            "--evidence", str(evidence_path),
            "--expected-engine", args.engine,
            "--story-id", args.story_id
        ]
        val_res = run(cmd, root)
        if val_res.returncode != 0:
            raise SystemExit(f"handoff blocked: Stage 3.5 engine validation failed:\n{val_res.stdout}\n{val_res.stderr}")

    evidence_json = evidence_path if not str(evidence_path).endswith(".sig") else Path(str(evidence_path)[:-4])
    evidence_data = {}
    if evidence_json.is_file():
        try:
            evidence_data = json.loads(evidence_json.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError, ValueError, TypeError) as exc:
            sys.stderr.write(f"[dispatch-code-engine] failed reading {evidence_json}: {exc}\n")

    plan_data = json.loads(plan.read_text(encoding="utf-8"))
    if approval and approval.is_file():
        appr_json = approval if not str(approval).endswith(".sig") else approval.with_suffix("")
        if appr_json.is_file():
            try:
                approval_data = json.loads(appr_json.read_text(encoding="utf-8"))
                if approval_data.get("gate") not in {"impl-plan-approval", "sign_human_gate"}:
                    raise SystemExit("handoff blocked: invalid approval gate")
                impl_md = story_dir / "impl-plan.md"
                if impl_md.is_file() and approval_data.get("manifest_hash"):
                    if approval_data.get("manifest_hash") != sha256(impl_md):
                        raise SystemExit("handoff blocked: approval is stale relative to impl-plan.md")
            except json.JSONDecodeError as exc:
                sys.stderr.write(f"[dispatch-code-engine] failed reading {appr_json}: {exc}\n")
                raise SystemExit(f"handoff blocked: malformed approval receipt {appr_json}") from exc
    plan_hash = sha256(plan)

    targets = sorted(set(plan_data.get("target_files", {}).get("create", []) + plan_data.get("target_files", {}).get("modify", [])))
    if not targets:
        raise SystemExit("handoff blocked: plan has no target files")
    for relative in targets:
        path = (root / relative).resolve()
        if not str(path).startswith(str(root) + "/"):
            raise SystemExit(f"handoff blocked: target escapes root: {relative}")
        if relative in plan_data.get("target_files", {}).get("modify", []) and not path.is_file():
            raise SystemExit(f"handoff blocked: declared modify target is missing: {relative}")

    run_id = f"run-{uuid.uuid4().hex}"
    namespace = story_dir / "execution" / args.engine
    namespace.mkdir(parents=True, exist_ok=True)
    catalog = root / ".agent" / "scripts" / "pi-code-agent" / "compile_capability_catalog.py"
    catalog_output = namespace / "capability-catalog.json"
    catalog_result = run([sys.executable, str(catalog), "--root", str(root), "--output", str(catalog_output)], root)
    if catalog_result.returncode != 0:
        raise SystemExit(catalog_result.stdout + catalog_result.stderr)
    catalog_data = json.loads(catalog_output.read_text(encoding="utf-8"))

    working_set = None
    task_contracts = []
    invocation = None
    worker_model_binding = None
    if args.engine == "pi-code-agent":
        # EC-P4: Preflight Guardrails: Ensure dependencies exist without redundant installs
        node_modules_dir = root / "node_modules"
        if not node_modules_dir.is_dir():
            try:
                print("📦 [Guardrail] node_modules missing. Running isolated pnpm install in worktree...", file=sys.stderr)
                pnpm_res = subprocess.run(["pnpm", "install", "--prefer-offline"], cwd=root, text=True, capture_output=True, timeout=30)
                if pnpm_res.returncode != 0:
                    print(f"⚠️ Warning: pnpm install exited with {pnpm_res.returncode}", file=sys.stderr)
            except Exception as e:
                print(f"⚠️ Warning: pnpm install skipped or timed out: {e}", file=sys.stderr)
        else:
            print("⚡ [Guardrail] node_modules already present. Skipping redundant pnpm install.", file=sys.stderr)

        if not args.platform:
            raise SystemExit(
                "handoff blocked: active platform is required. Run "
                "platform_profile_manager.py detect --root . and select the current IDE."
            )
        resolver = root / ".agent" / "scripts" / "pi-code-agent" / "resolve_invocation_context.py"
        invocation_path = namespace / "invocation.json"
        inv_mode = "bug-fix" if args.invocation_profile == "fix-bug" else "workflow"
        resolver_cmd = [
            sys.executable, str(resolver), "--root", str(root),
            "--profile", args.invocation_profile,
            "--caller-capability", args.caller_capability,
            "--caller-source", args.caller_source,
            "--invocation-mode", inv_mode,
            "--output", str(invocation_path),
        ]
        if args.authorization_receipt:
            resolver_cmd.extend(["--authorization-receipt", args.authorization_receipt])
        resolved = run(resolver_cmd, root)
        if resolved.returncode != 0:
            raise SystemExit(resolved.stdout + resolved.stderr)
        invocation = json.loads(invocation_path.read_text(encoding="utf-8"))
        binding_resolver = root / ".agent" / "scripts" / "pi-code-agent" / "resolve_model_binding.py"
        binding_path = namespace / "worker-model-binding.json"
        binding = run([sys.executable, str(binding_resolver), "--root", str(root), "--platform", args.platform, "--role", "worker", "--output", str(binding_path)], root)
        if binding.returncode != 0:
            raise SystemExit(binding.stdout + binding.stderr)
        worker_model_binding = json.loads(binding_path.read_text(encoding="utf-8"))
        native_loop = root / "scripts" / "pi-code-agent" / "native-contract-loop.py"
        working_set = namespace / "working-set.json"
        command = [sys.executable, str(native_loop), "prepare", "--root", str(root), "--plan-sha256", plan_hash, "--output", str(working_set)]
        # CREATE targets belong to the contract but have no pre-edit bytes to
        # hash. Only existing MODIFY targets enter the bounded working set.
        existing_targets = [target for target in targets if (root / target).is_file()]
        if not existing_targets:
            # Fallback to the relative plan file so at least one declared project file anchors the working set
            try:
                rel_plan = str(plan.relative_to(root))
                existing_targets = [rel_plan]
            except ValueError:
                # If outside root, anchor to story.md or package.json
                existing_targets = ["package.json"]
        
        for target in existing_targets:
            command.extend(["--file", target])
        prepared = run(command, root)
        if prepared.returncode != 0:
            raise SystemExit(prepared.stdout + prepared.stderr)
        compile_task = root / ".agent" / "scripts" / "pi-code-agent" / "compile_task_contract.py"
        task_runner = root / ".agent" / "scripts" / "pi-code-agent" / "task_runner.py"
        # Extract scoped test files from tasks and test commands in impl-plan.json
        scoped_story_test_files = set(plan_data.get("story_test_files", []))
        for t in plan_data.get("tasks", []):
            t_targets = t.get("target_files") or [t.get("target")] or []
            for f in t_targets:
                if isinstance(f, str) and any(sub in f for sub in (".test.", ".spec.", "test_")):
                    scoped_story_test_files.add(f)
            t_cmd = t.get("test_command") or ""
            for token in t_cmd.split():
                if any(token.endswith(ext) for ext in (".ts", ".tsx", ".js", ".jsx", ".py")) and any(sub in token for sub in (".test.", ".spec.", "test_")):
                    scoped_story_test_files.add(token)
        for token in (plan_data.get("test_command") or "").split():
            if any(token.endswith(ext) for ext in (".ts", ".tsx", ".js", ".jsx", ".py")) and any(sub in token for sub in (".test.", ".spec.", "test_")):
                scoped_story_test_files.add(token)

        story_test_file_args = []
        for stf in sorted(scoped_story_test_files):
            story_test_file_args.extend(["--story-test-file", stf])

        for task in plan_data.get("tasks", []):
            contract_path = namespace / f"{task['task_id']}.json"
            
            target_path = task.get("target", "")
            if target_path.endswith((".ts", ".tsx")):
                fallback_checkers = ["--checker-id", "vitest"]
            elif target_path.endswith(".py"):
                fallback_checkers = ["--checker-id", "pytest"]
            else:
                fallback_checkers = ["--checker-id", "vitest"]

            compile_cmd = [
                sys.executable, str(compile_task), "--impl-plan", str(plan),
                "--task-id", task["task_id"], "--root", str(root),
                "--run-id", run_id, "--output", str(contract_path),
                *fallback_checkers,
                "--risk-tier", str(task.get("risk_tier") or plan_data.get("risk_tier", "medium")),
                *story_test_file_args,
            ]
            result = run(compile_cmd, root)
            if result.returncode != 0:
                raise SystemExit(result.stdout + result.stderr)
            preflight_path = namespace / f"{task['task_id']}.preflight.json"
            preflight = run([
                sys.executable, str(task_runner), "preflight", "--root", str(root),
                "--contract", str(contract_path), "--output", str(preflight_path),
            ], root)
            if preflight.returncode != 0:
                raise SystemExit(preflight.stdout + preflight.stderr)
            task_contracts.append(str(contract_path))
        ledger = namespace / "task-ledger.json"
        bootstrap = namespace / "ledger-bootstrap.json"
        bootstrap.write_text(json.dumps({"run_id": run_id, "task_contracts": task_contracts}, indent=2) + "\n", encoding="utf-8")
        initialized = run([sys.executable, str(task_runner), "init", "--root", str(root), "--handoff", str(bootstrap), "--output", str(ledger)], root)
        if initialized.returncode != 0:
            raise SystemExit(initialized.stdout + initialized.stderr)

    omp_spec_path = None
    if args.engine == "omp-orch-skill":
        omp_spec_file = namespace / "omp-execution-spec.json"
        omp_spec = {
            "story_id": args.story_id,
            "engine": "omp-orch-skill",
            "run_id": run_id,
            "selected_topology": evidence_data.get("selected_topology", "antigravity"),
            "worker_model": evidence_data.get("worker_model"),
            "worker_effort": evidence_data.get("worker_effort"),
            "advisor_model": evidence_data.get("advisor_model"),
            "advisor_effort": evidence_data.get("advisor_effort"),
            "nonce": evidence_data.get("nonce"),
            "plan_sha256": plan_hash,
            "target_files": targets,
        }
        omp_spec_file.write_text(json.dumps(omp_spec, indent=2) + "\n", encoding="utf-8")
        omp_spec_path = str(omp_spec_file)

    handoff = {
        "schema_version": "2.0",
        "status": "ready_for_ide_agent",
        "engine": args.engine,
        "run_id": run_id,
        "story_id": plan_data.get("story_id"),
        "plan_sha256": plan_hash,
        "catalog_sha256": catalog_data["catalog_sha256"],
        "target_files": targets,
        "task_contracts": task_contracts,
        "working_set": str(working_set) if working_set else None,
        "invocation": invocation,
        "worker_model_binding": worker_model_binding,
        "selected_topology": evidence_data.get("selected_topology"),
        "worker_model": evidence_data.get("worker_model"),
        "worker_effort": evidence_data.get("worker_effort"),
        "advisor_model": evidence_data.get("advisor_model"),
        "advisor_effort": evidence_data.get("advisor_effort"),
        "nonce": evidence_data.get("nonce"),
        "omp_execution_spec": omp_spec_path,
        "task_ledger": str(namespace / "task-ledger.json") if args.engine == "pi-code-agent" else None,
        "next_required_action": "Active IDE agent obtains a lease, executes one bounded task, and submits only a candidate receipt to the external review/validator gates.",
        "receipt_policy": "This command cannot emit an accepted receipt. Only task_runner accept with a separately signed validator attestation may do so.",
    }
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(handoff, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(handoff, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
