#
# @story-41.2
#!/usr/bin/env python3
"""Host-neutral preparation and evidence gates for the native pi-code-agent path.

This utility does not execute model-provided commands. It prepares immutable
inputs, validates receipts through the iWish kernel, and emits learning
candidates that remain inactive until the governed /skill flow promotes them.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / ".agent" / "scripts" / "pi-code-agent"))
from core import (ContractError, atomic_json_write, canonical, compile_catalog,
                  validate_catalog, validate_receipt, validate_receipt_namespace)


def digest(value: object) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def command_prepare(args: argparse.Namespace) -> int:
    root = ROOT if args.root is None else Path(args.root).resolve()
    catalog = compile_catalog(root)
    validate_catalog(catalog)
    files = sorted(set(args.file))
    if not files:
        try:
            plan_file = Path(args.output).resolve().parents[2] / "impl-plan.json"
            if plan_file.is_file():
                try:
                    files = [str(plan_file.relative_to(root))]
                except ValueError:
                    pass
        except (OSError, IndexError) as exc:
            sys.stderr.write(f"[native-contract-loop] failed resolving impl-plan.json fallback: {exc}\n")
    if not files:
        raise ContractError("at least one declared target file is required")
    for relative in files:
        path = (root / relative).resolve()
        if not str(path).startswith(str(root) + "/") or not path.is_file():
            raise ContractError(f"target file is outside the project or missing: {relative}")
    manifest = {
        "schema_version": "1.0",
        "manifest_id": "working-set-" + digest([args.plan_sha256, files])[:16],
        "catalog_sha256": catalog["catalog_sha256"],
        "plan_sha256": args.plan_sha256,
        "sources": [{"path": relative, "sha256": hashlib.sha256((root / relative).read_bytes()).hexdigest()} for relative in files],
        "stable_blocks": ["project-policy", "approved-plan", "catalog", "role-policy"],
        "volatile_blocks": ["current-task", "diagnostics", "prior-receipts"],
        "token_budget": args.token_budget,
        "prompt_assembly_mode": "references-first-bounded-body-load",
    }
    atomic_json_write(Path(args.output), manifest)
    print(json.dumps({"manifest": args.output, "manifest_sha256": digest(manifest), "catalog_sha256": catalog["catalog_sha256"]}, indent=2))
    return 0


def command_validate_receipt(args: argparse.Namespace) -> int:
    receipt = json.loads(Path(args.receipt).read_text(encoding="utf-8"))
    validate_receipt_namespace(receipt, set(args.existing_id))
    print(json.dumps({"receipt": args.receipt, "structurally_valid": True, "authoritative": False, "note": "ledger acceptance is required before Stage 4 or learning"}, indent=2))
    return 0


def command_learn(args: argparse.Namespace) -> int:
    receipt = json.loads(Path(args.receipt).read_text(encoding="utf-8"))
    validate_receipt(receipt)
    if receipt.get("receipt_kind") != "validator-accepted":
        raise ContractError("learning requires a ledger-produced validator-accepted receipt")
    candidate = {
        "schema_version": "1.0",
        "candidate_id": "candidate-" + hashlib.sha256(receipt["receipt_id"].encode()).hexdigest()[:16],
        "status": "candidate",
        "source_receipt_ids": [receipt["receipt_id"]],
        "source_run_id": receipt["run_id"],
        "source_task_id": receipt["task_id"],
        "proposal": args.proposal,
        "activation": "requires-evaluation-and-explicit-skill-promotion",
        "active": False,
    }
    atomic_json_write(Path(args.output), candidate)
    print(json.dumps({"candidate": args.output, "active": False}, indent=2))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    prepare = sub.add_parser("prepare")
    prepare.add_argument("--root")
    prepare.add_argument("--plan-sha256", required=True)
    prepare.add_argument("--file", action="append", default=[])
    prepare.add_argument("--token-budget", type=int, default=12000)
    prepare.add_argument("--output", required=True)
    receipt = sub.add_parser("validate-receipt")
    receipt.add_argument("--receipt", required=True)
    receipt.add_argument("--existing-id", action="append", default=[])
    learn = sub.add_parser("learn")
    learn.add_argument("--receipt", required=True)
    learn.add_argument("--proposal", required=True)
    learn.add_argument("--output", required=True)
    args = parser.parse_args()
    if args.command == "prepare" and not re.fullmatch(r"[a-f0-9]{64}", args.plan_sha256):
        parser.error("--plan-sha256 must be a 64-character lowercase SHA-256")
    try:
        return {"prepare": command_prepare, "validate-receipt": command_validate_receipt, "learn": command_learn}[args.command](args)
    except (ContractError, OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"CONTRACT BLOCKED: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
