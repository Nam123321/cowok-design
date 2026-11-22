#!/usr/bin/env python3
# @story-88.06
"""
Zero-Trust Read-Only Checkpoint Gate for Story Completion.
Strictly enforces:
  1. Read-only file opens with os.O_RDONLY | os.O_NOFOLLOW (rejects symlinks, non-regular files, and 0-byte stubs).
  2. Exact token boundary matching for story_id in sprint-status.yaml (single-line AND multi-line nested YAML) + story.md frontmatter.
  3. Cryptographic Watchmen signature verification + Stage 4 Authentic Review Evidence Chain verification.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from pathlib import Path
from typing import Any, Optional, Tuple

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SCRIPTS_DIR = PROJECT_ROOT / ".agent" / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))


def report_validation_failure(story_id: str, reason: str) -> None:
    """100% Read-Only Fail-Closed alert. Never mutates files on disk."""
    print(f"\n[ZERO-TRUST GATE FAIL] Story {story_id} validation failed: {reason}", file=sys.stderr)
    print(
        "[ZERO-TRUST GATE FAIL] Run /review or /approve-qa to establish valid cryptographic review evidence.\n",
        file=sys.stderr,
    )


def _safe_read_regular_bytes(path: Path, max_bytes: int = 8 * 1024 * 1024) -> Optional[bytes]:
    """Read file bytes strictly using O_RDONLY | O_NOFOLLOW; reject symlinks, empty files, and hardlinks > 1."""
    raw_str = str(path)
    if os.path.islink(raw_str):
        return None
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(raw_str, flags)
    except OSError:
        return None
    try:
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode):
            return None
        if st.st_size <= 0 or st.st_size > max_bytes:
            return None
        if getattr(st, "st_nlink", 1) > 1:
            return None
        chunks = []
        remaining = st.st_size
        while remaining > 0:
            chunk = os.read(fd, min(65536, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        return data if len(data) > 0 else None
    except OSError as exc:
        sys.stderr.write(f"[validate-story-completion-gate] ERROR: failed reading {path}: {exc}\n")
        return None
    finally:
        try:
            os.close(fd)
        except OSError:
            pass


def _is_story_marked_completed(story_id: str, project_root: Path) -> bool:
    """Check canonical sprint-status.yaml (single-line AND multi-line nested YAML) + story.md frontmatter."""
    escaped_id = re.escape(story_id)
    slug_id = re.escape(f"story-{story_id.replace('.', '-')}")
    dot_slug_id = re.escape(f"story-{story_id}")
    story_pattern = re.compile(
        rf"(?<![\d.\-a-zA-Z])(?:Story\s+{escaped_id}|{slug_id}|{dot_slug_id}|['\"]?{escaped_id}['\"]?)(?![\d.\-a-zA-Z])",
        re.IGNORECASE,
    )
    completed_val_pattern = re.compile(r":\s*['\"]?completed['\"]?(?:\s*(?:#.*)?)?$", re.IGNORECASE)
    nested_key_pattern = re.compile(
        rf"^(\s*)(?:['\"]?(?:Story\s+{escaped_id}|{slug_id}|{dot_slug_id}|{escaped_id})['\"]?)\s*:\s*(?:#.*)?$",
        re.IGNORECASE,
    )
    status_completed_pattern = re.compile(
        r"^\s*status\s*:\s*['\"]?completed['\"]?(?:\s*(?:#.*)?)?$",
        re.IGNORECASE | re.MULTILINE,
    )

    sprint_candidates = [
        project_root / "_iwish-output" / "sprint-status.yaml",
        project_root / "_iwish-output" / "3. Development" / "sprint-status.yaml",
    ]
    for sp in sprint_candidates:
        raw = _safe_read_regular_bytes(sp)
        if not raw:
            continue
        text = raw.decode("utf-8", errors="replace")
        lines = text.splitlines()
        in_story_block = False
        story_indent = 0
        for line in lines:
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            # (a) Single-line match: e.g. `story-01-1: completed` or `'01.1': completed`
            if story_pattern.search(stripped) and completed_val_pattern.search(stripped):
                return True
            # (b) Multi-line nested YAML match:
            #     '01.1':
            #       title: ...
            #       status: completed
            m_key = nested_key_pattern.match(line)
            if m_key:
                in_story_block = True
                story_indent = len(m_key.group(1))
                continue
            if in_story_block:
                cur_indent = len(line) - len(line.lstrip())
                if cur_indent <= story_indent:
                    in_story_block = False
                elif status_completed_pattern.match(line):
                    return True

    # (c) Also check Story-<story_id>/story.md frontmatter `status: completed`
    epic_story_root = project_root / "_iwish-output" / "3. Development" / "1. Epic & Story"
    if epic_story_root.exists():
        try:
            for p in epic_story_root.rglob(f"Story-{story_id}"):
                if p.is_dir() and not os.path.islink(str(p)):
                    story_md = p / "story.md"
                    raw_md = _safe_read_regular_bytes(story_md)
                    if raw_md:
                        md_text = raw_md.decode("utf-8", errors="replace")
                        fm_match = re.match(r"^---\s*\n(.*?)\n---", md_text, re.DOTALL)
                        if fm_match and status_completed_pattern.search(fm_match.group(1)):
                            status_lines = [
                                ln for ln in fm_match.group(1).splitlines() if status_completed_pattern.match(ln)
                            ]
                            if status_lines:
                                return True
        except OSError as exc:
            sys.stderr.write(
                f"[validate-story-completion-gate] WARN: failed scanning {epic_story_root} for Story-{story_id}: {exc}\n"
            )

    return False


def _verify_watchmen_signature(payload_without_sig: dict[str, Any], signature: str) -> bool:
    """Verify Watchmen HMAC/daemon signature in a fail-closed manner (supports both JSON payload digest and gate:timestamp:manifest_hash digest)."""
    if not signature or not isinstance(signature, str) or len(signature.strip()) < 16:
        return False
    sig_clean = signature.strip()
    nonce = payload_without_sig.get("nonce")
    if not nonce or not isinstance(nonce, str):
        return False
    try:
        import watchmen_client  # type: ignore

        # Scheme 1: Full JSON payload digest (`watchmen_client.sign_evidence`)
        if bool(watchmen_client.verify_evidence(payload_without_sig, sig_clean)):
            return True

        # Scheme 2: Watchmen MCP `sign_human_gate` / `sign_pipeline_gate` digest (`${gate}:${timestamp}:${manifest_hash}`)
        gate = payload_without_sig.get("gate")
        ts = payload_without_sig.get("timestamp")
        m_hash = payload_without_sig.get("manifest_hash")
        if gate and ts is not None and m_hash:
            mcp_digest = hashlib.sha256(f"{gate}:{ts}:{m_hash}".encode("utf-8")).hexdigest()
            res = watchmen_client.call_daemon(
                "verify_signature",
                {"nonce": nonce, "digest": mcp_digest, "signature": sig_clean},
            )
            if isinstance(res, dict) and res.get("success") is True and not res.get("error"):
                return True
        return False
    except SystemExit:
        return False
    except (ImportError, OSError, RuntimeError, ValueError, TypeError) as exc:
        sys.stderr.write(f"[validate-story-completion-gate] ERROR: Watchmen signature verification error: {exc}\n")
        return False


def _compute_story_spec_manifest_hash(story_dir: Path) -> str:
    """Compute NFC-normalized SHA-256 manifest hash over story spec files (excluding post-Stage-4 qa-approved receipts)."""
    import unicodedata

    if not story_dir.is_dir():
        return ""
    exclude_names = {
        "qa-approved_evidence.json",
        "qa-approved-evidence.json",
        "qa-approved_evidence.json.sig",
        "qa-approved-evidence.json.sig",
    }
    manifest_lines: list[str] = []
    for f in sorted(story_dir.iterdir(), key=lambda p: p.name):
        if f.is_file() and not f.is_symlink() and f.name not in exclude_names and not f.name.endswith(".sig"):
            raw_f = _safe_read_regular_bytes(f) or b""
            norm_txt = unicodedata.normalize(
                "NFC",
                raw_f.decode("utf-8", errors="replace").replace("\r\n", "\n").strip(),
            )
            manifest_lines.append(f"{f.name}:{hashlib.sha256(norm_txt.encode('utf-8')).hexdigest()}\n")
    return hashlib.sha256("".join(manifest_lines).encode("utf-8")).hexdigest().lower()


def _resolve_story_contract_targets(story_dir: Path) -> set[str]:
    """Resolve contract target files from impl-plan.json, traceability.json, and story/task/impl-plan markdown."""
    targets: set[str] = set()
    generic_dirs = {
        "src", "src/", "src/app", "src/app/", "src/app/api", "src/app/api/",
        "src/features", "src/features/", "src/api", "src/api/", "prisma", "prisma/",
        "tests", "tests/", "packages", "packages/", "apps", "apps/", "crates", "crates/",
    }

    def _add(p: Any) -> None:
        s = os.path.normpath(str(p or "").strip()).lstrip("./")
        if not s or s in generic_dirs or s.endswith("/") or any(c in s for c in "*?<>"):
            return
        targets.add(s)

    plan_json = story_dir / "impl-plan.json"
    raw_pj = _safe_read_regular_bytes(plan_json)
    if raw_pj:
        try:
            pj = json.loads(raw_pj.decode("utf-8"))
            tf = pj.get("target_files") or {}
            if isinstance(tf, dict):
                for k in ("modify", "create", "delete"):
                    for item in tf.get(k) or []:
                        _add(item)
        except (UnicodeDecodeError, json.JSONDecodeError, ValueError, TypeError) as exc:
            sys.stderr.write(f"[validate-story-completion-gate] WARN: Invalid impl-plan.json {plan_json}: {exc}\n")

    trace_json = story_dir / "traceability.json"
    raw_tj = _safe_read_regular_bytes(trace_json)
    if raw_tj:
        try:
            tj = json.loads(raw_tj.decode("utf-8"))
            for m in tj.get("mappings") or tj.get("ac_mappings") or []:
                if isinstance(m, dict):
                    for item in m.get("files") or m.get("code_files") or m.get("test_files") or []:
                        _add(item)
            for item in tj.get("files") or tj.get("implementation_files") or []:
                _add(item)
        except (UnicodeDecodeError, json.JSONDecodeError, ValueError, TypeError) as exc:
            sys.stderr.write(f"[validate-story-completion-gate] WARN: Invalid traceability.json {trace_json}: {exc}\n")

    for md_name in ("impl-plan.md", "story.md", "task.md"):
        raw_md = _safe_read_regular_bytes(story_dir / md_name)
        if raw_md:
            md_txt = raw_md.decode("utf-8", errors="replace")
            for m in re.finditer(r"`((?:src|prisma|tests|packages|apps|crates)/[^`\s]+\.[a-zA-Z0-9]+)`", md_txt):
                _add(m.group(1))
    return targets


def _verify_live_target_files_match(project_root: Path, story_dir: Path, ev_data: dict[str, Any]) -> bool:
    """Verify that target_files covers all existing contract targets and target_files_sha256 matches live disk."""
    expected_sha = str(ev_data.get("target_files_sha256") or "").strip().lower()
    target_files = ev_data.get("target_files")
    if len(expected_sha) != 64 or not isinstance(target_files, list) or not target_files:
        return False
    cleaned = sorted(
        set(os.path.normpath(str(tf).strip()).lstrip("./") for tf in target_files if str(tf).strip())
    )
    if not cleaned:
        return False
    declared_set = set(cleaned)
    root_resolved = project_root.resolve()
    for ct in _resolve_story_contract_targets(story_dir):
        full_ct = (root_resolved / ct).resolve()
        if full_ct.is_file() and ct not in declared_set:
            return False
    entries: list[str] = []
    for rel in cleaned:
        full = (root_resolved / rel).resolve()
        if not str(full).startswith(str(root_resolved) + os.sep):
            return False
        raw_f = _safe_read_regular_bytes(full)
        if raw_f is None:
            return False
        entries.append(f"{rel}:{hashlib.sha256(raw_f).hexdigest()}")
    live_sha = hashlib.sha256("\n".join(entries).encode("utf-8")).hexdigest().lower()
    return live_sha == expected_sha


def _verify_fallback_receipt_and_substitute_review(
    raw_fb: bytes, fallback_path: Path, story_dir: Path, project_root: Path, story_id: str
) -> Tuple[bool, str]:
    """Validate that codex-review-fallback-<story_id>.json has valid schema/HMAC and is accompanied by a cryptographically signed APPROVED substitute review bound to live state."""
    try:
        fb_data = json.loads(raw_fb.decode("utf-8"))
    except Exception as exc:
        return False, f"Stage 4 Chain Violation: corrupted fallback receipt {fallback_path.name} ({exc})"
    if not isinstance(fb_data, dict):
        return False, f"Stage 4 Chain Violation: fallback receipt {fallback_path.name} must be a JSON object"

    status = str(fb_data.get("status") or "").strip().upper()
    fb_story = str(fb_data.get("story_id") or "").strip().lower().removeprefix("story-")
    norm_story = story_id.strip().lower().removeprefix("story-")
    reason = str(fb_data.get("reason") or "").strip()
    ts = str(fb_data.get("timestamp") or "").strip()
    fb_platform = str(fb_data.get("fallback_platform") or "").strip().lower()

    if status != "FALLBACK_TRIGGERED" or fb_story != norm_story or not reason or not ts or fb_platform != "antigravity":
        return (
            False,
            f"Stage 4 Chain Violation: invalid fallback receipt schema in {fallback_path.name} "
            f"(expected status='FALLBACK_TRIGGERED', story_id='{norm_story}', fallback_platform='antigravity', non-empty reason/timestamp)",
        )

    # Verify HMAC-SHA256 signature on fallback receipt (fail-closed)
    rec_hmac = fb_data.get("receipt_hmac")
    if not rec_hmac or not isinstance(rec_hmac, str) or len(rec_hmac) != 64:
        return False, f"Stage 4 Chain Violation: fallback receipt {fallback_path.name} is missing 64-char cryptographic 'receipt_hmac'"
    try:
        from ledger_hmac import verify_hmac  # type: ignore

        verify_copy = dict(fb_data)
        verify_copy.pop("receipt_hmac", None)
        canonical_fb = json.dumps(verify_copy, sort_keys=True, separators=(",", ":"))
        if not verify_hmac(canonical_fb, rec_hmac):
            return False, f"Stage 4 Chain Violation: fallback receipt {fallback_path.name} failed HMAC-SHA256 integrity verification"
    except Exception as exc:
        return False, f"Stage 4 Chain Violation: fallback receipt HMAC verification failed ({exc})"

    # Require cryptographically signed substitute Antigravity review evidence bound to the same story_id AND LIVE workspace state (never allow intra-story or cross-story replay)
    reviews_dir = project_root / "_iwish-output" / "reviews"
    story_manifest_hash = _compute_story_spec_manifest_hash(story_dir)

    ag_ev_path = reviews_dir / f"review-ag-evidence-{story_id}.json"
    raw_ag = _safe_read_regular_bytes(ag_ev_path)
    if raw_ag is not None:
        try:
            from ledger_hmac import verify_hmac  # type: ignore

            ag_data = json.loads(raw_ag.decode("utf-8"))
            ag_story = str(ag_data.get("story_id") or ag_data.get("target_id") or "").strip().lower().removeprefix("story-")
            ag_verdict = str(ag_data.get("verdict") or ag_data.get("status") or "").upper()
            ag_mhash = str(ag_data.get("manifest_hash") or "").strip().lower()
            ag_ts = str(ag_data.get("timestamp") or "").strip()
            ag_hmac = str(ag_data.get("receipt_hmac") or "").strip()
            ag_copy = dict(ag_data)
            ag_copy.pop("receipt_hmac", None)
            ag_sig = str(ag_data.get("signature") or ag_data.get("Signature") or "").strip()
            ag_sig_copy = dict(ag_data)
            ag_sig_copy.pop("signature", None)
            ag_sig_copy.pop("Signature", None)
            is_ag_signed = (len(ag_hmac) == 64 and verify_hmac(json.dumps(ag_copy, sort_keys=True, separators=(",", ":")), ag_hmac)) or (
                bool(ag_sig) and _verify_watchmen_signature(ag_sig_copy, ag_sig)
            )
            has_valid_live_binding = (
                bool(ag_mhash)
                and ag_mhash == story_manifest_hash
                and _verify_live_target_files_match(project_root, story_dir, ag_data)
            )
            is_fresh = bool(ag_ts) and ag_ts >= ts
            if ag_story == norm_story and ag_verdict in ("APPROVED", "PASS", "CLEAN") and is_ag_signed and is_fresh and has_valid_live_binding:
                return True, "Valid fallback receipt + cryptographically signed APPROVED review-ag-evidence bound to story, manifest, and live target_files"
        except (ImportError, OSError, UnicodeDecodeError, json.JSONDecodeError, ValueError, TypeError, RuntimeError) as exc:
            sys.stderr.write(f"[validate-story-completion-gate] WARN: Invalid Antigravity review evidence {ag_ev_path}: {exc}\n")

    review_md_path = reviews_dir / f"review-story-{story_id}.md"
    raw_md = _safe_read_regular_bytes(review_md_path)
    if raw_md is not None:
        md_text = raw_md.decode("utf-8", errors="replace")
        mentions_story = bool(re.search(rf"(?<![0-9A-Za-z._-]){re.escape(norm_story)}(?![0-9A-Za-z._-])", md_text, re.IGNORECASE))
        if (
            mentions_story
            and re.search(r"\b(?:VERDICT|STATUS)\s*:\s*`?(?:APPROVED|PASS|PASSED)`?", md_text, re.IGNORECASE)
            and _verify_live_target_files_match(project_root, story_dir, fb_data)
        ):
            valid_mhashes = {story_manifest_hash} if story_manifest_hash else set()
            signed_gate_candidates = [
                reviews_dir / "validate-review-evidence_evidence.json",
                story_dir / "validate-review-evidence_evidence.json",
                story_dir / "stage-4-review-evidence.json",
            ]
            for cand in signed_gate_candidates:
                raw_cand = _safe_read_regular_bytes(cand)
                if raw_cand is None:
                    continue
                try:
                    c_data = json.loads(raw_cand.decode("utf-8"))
                    c_story = str(c_data.get("story_id") or c_data.get("target_id") or c_data.get("story") or "").strip().lower().removeprefix("story-")
                    c_mhash = str(c_data.get("manifest_hash") or "").strip().lower()
                    if c_story and c_story != norm_story:
                        continue
                    if c_mhash not in valid_mhashes:
                        continue
                    c_sig = str(c_data.get("signature") or c_data.get("Signature") or "").strip()
                    c_copy = dict(c_data)
                    c_copy.pop("signature", None)
                    c_copy.pop("Signature", None)
                    if c_sig and _verify_watchmen_signature(c_copy, c_sig):
                        return True, "Valid fallback receipt + APPROVED review-story markdown with story-bound and manifest-bound signed Watchmen gate evidence"
                except (OSError, UnicodeDecodeError, json.JSONDecodeError, ValueError, TypeError, RuntimeError) as exc:
                    sys.stderr.write(f"[validate-story-completion-gate] WARN: Failed verifying signed gate candidate {cand}: {exc}\n")
                    continue

    return (
        False,
        f"Stage 4 Chain Violation: fallback receipt {fallback_path.name} present, but cryptographically signed substitute Antigravity review bound to Story {story_id} and live state "
        f"('{ag_ev_path.name}' with valid HMAC + live manifest + live target_files_sha256 or APPROVED '{review_md_path.name}' with story-bound signed Watchmen gate evidence) is missing or invalid",
    )


def _verify_stage4_review_chain(story_dir: Path, project_root: Path, story_id: str) -> Tuple[bool, str]:
    """Ensure that if review-engine-selection.json specifies tournament or codex, authentic HMAC-signed review-codex-evidence-<id>.json exists, passed, and matches live manifest + target_files_sha256."""
    norm_story = story_id.strip().lower().removeprefix("story-")
    if not norm_story or norm_story in ("adhoc", "unknown", "none") or not story_dir.is_dir():
        return False, f"Stage 4 Chain Violation: Fail-Closed on invalid or non-story target '{story_id}'"

    sel_path = story_dir / "review-engine-selection.json"
    raw_sel = _safe_read_regular_bytes(sel_path)
    selected_engine = "codex"
    if raw_sel is not None:
        try:
            sel_data = json.loads(raw_sel.decode("utf-8"))
            selected_engine = str(sel_data.get("selected_engine") or sel_data.get("selected_option") or "codex").lower()
        except Exception as exc:
            return False, f"Invalid review-engine-selection.json: {exc}"

    if "tournament" in selected_engine or "codex" in selected_engine:
        codex_ev_path = project_root / "_iwish-output" / "reviews" / f"review-codex-evidence-{story_id}.json"
        fallback_path = project_root / "_iwish-output" / "adhoc-workspace" / "scratch" / f"codex-review-fallback-{story_id}.json"
        raw_codex = _safe_read_regular_bytes(codex_ev_path)
        raw_fb = _safe_read_regular_bytes(fallback_path)
        if raw_codex is None and raw_fb is None:
            return (
                False,
                f"Stage 4 Chain Violation: review-engine-selection chose '{selected_engine}', but '{codex_ev_path.name}' is missing (hand-crafted review-story-{story_id}.md without Codex execution is rejected)",
            )
        if raw_codex is not None:
            try:
                from ledger_hmac import verify_hmac  # type: ignore

                codex_data = json.loads(raw_codex.decode("utf-8"))
                ev_story = str(codex_data.get("story_id") or "").strip().lower().removeprefix("story-")
                verdict = str(codex_data.get("verdict") or codex_data.get("status") or "").upper()
                diff_hash = str(codex_data.get("diff_hash") or "").upper()
                rec_hmac = str(codex_data.get("receipt_hmac") or "").strip()
                ev_mhash = str(codex_data.get("manifest_hash") or "").strip().lower()
                if ev_story != norm_story:
                    return False, f"Stage 4 Chain Violation: {codex_ev_path.name} story_id='{ev_story}' does not match '{norm_story}'"
                if verdict not in ("APPROVED", "PASS", "CLEAN"):
                    return False, f"Stage 4 Chain Violation: {codex_ev_path.name} verdict is '{verdict}' (expected APPROVED)"
                if not diff_hash or diff_hash == "EMPTY":
                    return False, f"Stage 4 Chain Violation: {codex_ev_path.name} has diff_hash='{diff_hash}' (empty diff review rejected)"
                verify_copy = dict(codex_data)
                verify_copy.pop("receipt_hmac", None)
                if len(rec_hmac) != 64 or not verify_hmac(json.dumps(verify_copy, sort_keys=True, separators=(",", ":")), rec_hmac):
                    return False, f"Stage 4 Chain Violation: {codex_ev_path.name} is missing or failed cryptographic HMAC-SHA256 'receipt_hmac' verification"
                live_story_mhash = _compute_story_spec_manifest_hash(story_dir)
                if not ev_mhash or ev_mhash != live_story_mhash:
                    return False, f"Stage 4 Chain Violation: {codex_ev_path.name} manifest_hash does not match live Story directory manifest"
                if not _verify_live_target_files_match(project_root, story_dir, codex_data):
                    return False, f"Stage 4 Chain Violation: {codex_ev_path.name} target_files_sha256 does not cover or match live contract target files on disk (stale review after code change rejected)"
            except Exception as exc:
                return False, f"Stage 4 Chain Violation: corrupted {codex_ev_path.name} ({exc})"
        elif raw_fb is not None:
            ok_fb, fb_msg = _verify_fallback_receipt_and_substitute_review(
                raw_fb, fallback_path, story_dir, project_root, story_id
            )
            if not ok_fb:
                return False, fb_msg

    return True, "Stage 4 review chain verified"


def _verify_pipeline_or_review_json(
    json_path: Path, expected_story_id: str, project_root: Optional[Path] = None
) -> Tuple[bool, str]:
    """Validate structured JSON evidence + target story_id + cryptographic Watchmen signature + Stage 4 chain."""
    raw = _safe_read_regular_bytes(json_path)
    if raw is None:
        return False, f"{json_path.name}: missing, 0-byte, symlink, or hardlink"

    try:
        data = json.loads(raw.decode("utf-8"))
    except Exception as exc:
        return False, f"{json_path.name}: invalid JSON ({exc})"

    if not isinstance(data, dict):
        return False, f"{json_path.name}: root JSON must be an object"

    if data.get("UNSIGNED_DEV") is True:
        return False, f"{json_path.name}: UNSIGNED_DEV=true is rejected in production completion gate"

    # Verify story_id match
    recorded_id = str(
        data.get("target_id")
        or data.get("story_id")
        or data.get("story")
        or ""
    ).strip()
    norm_expected = expected_story_id.lower().removeprefix("story-")
    norm_recorded = recorded_id.lower().removeprefix("story-")
    if not norm_recorded or norm_recorded != norm_expected:
        return False, f"{json_path.name}: story_id mismatch (expected '{expected_story_id}', got '{recorded_id}')"

    # Verify pass verdict
    verdict = str(data.get("verdict") or data.get("status") or "").upper()
    all_passed = data.get("all_passed")
    if all_passed is False:
        return False, f"{json_path.name}: all_passed is False"
    if all_passed is not True and verdict not in ("PASS", "PASSED", "APPROVED", "APPROVE", "PROVEN_SAFE", "CLEAN", "SUCCESS", "COMPLETED"):
        if "findings" in data and isinstance(data["findings"], list):
            blocking = [
                f for f in data["findings"]
                if isinstance(f, dict) and str(f.get("severity", "")).upper() in ("CRITICAL", "HIGH", "P0", "P1")
                and str(f.get("status", "OPEN")).upper() not in ("RESOLVED", "FIXED", "DISMISSED")
            ]
            if blocking:
                return False, f"{json_path.name}: contains {len(blocking)} unresolved blocking findings"
        elif data.get("coverage") and isinstance(data.get("coverage"), dict):
            failed = data["coverage"].get("failed") or []
            if failed:
                return False, f"{json_path.name}: review-run-manifest has failed files: {failed}"
        else:
            return False, f"{json_path.name}: missing explicit pass status/all_passed"

    # Verify Stage 4 Review Chain if inside a Story directory
    if project_root is not None:
        story_dir_cand = json_path.parent
        if story_dir_cand.name == "evidence" and story_dir_cand.parent.name == "qa":
            story_dir_cand = story_dir_cand.parent.parent
        if story_dir_cand.name.lower().startswith("story-"):
            s4_ok, s4_msg = _verify_stage4_review_chain(story_dir_cand, project_root, expected_story_id)
            if not s4_ok:
                return False, f"{json_path.name}: {s4_msg}"

    # Case A: Embedded Watchmen signature (e.g. pipeline-evidence-review.json / qa-approved-evidence.json)
    embedded_sig = data.get("hmac_signature") or data.get("signature")
    if isinstance(embedded_sig, str) and embedded_sig.strip():
        payload_copy = dict(data)
        payload_copy.pop("hmac_signature", None)
        payload_copy.pop("signature", None)

        # Verify evidence_hash integrity if present
        if "evidence_hash" in payload_copy:
            expected_ev_hash = payload_copy["evidence_hash"]
            base_copy = dict(payload_copy)
            base_copy.pop("evidence_hash", None)
            base_copy.pop("nonce", None)
            computed_ev_hash = hashlib.sha256(json.dumps(base_copy, sort_keys=True).encode("utf-8")).hexdigest()
            if computed_ev_hash != expected_ev_hash:
                return False, f"{json_path.name}: evidence_hash tampered"

        if _verify_watchmen_signature(payload_copy, embedded_sig):
            return True, f"Verified embedded Watchmen signature in {json_path}"
        return False, f"{json_path.name}: embedded Watchmen signature verification failed (missing valid nonce or forged signature)"

    # Case B: Detached .sig companion file
    sig_companion = Path(f"{json_path}.sig")
    sig_raw = _safe_read_regular_bytes(sig_companion)
    if sig_raw is None:
        return False, f"{json_path.name}: missing or invalid companion signature {sig_companion.name}"

    sig_text = sig_raw.decode("utf-8", errors="replace").strip()
    try:
        sig_obj = json.loads(sig_text)
        if isinstance(sig_obj, dict):
            sig_hex = str(sig_obj.get("signature") or sig_obj.get("hmac_signature") or "").strip()
            nonce = str(sig_obj.get("nonce") or data.get("nonce") or "").strip()
            digest = str(sig_obj.get("manifest_hash") or sig_obj.get("digest") or "").strip()
            actual_sha = hashlib.sha256(raw).hexdigest()
            if digest and digest != actual_sha:
                norm_sha = hashlib.sha256(raw.replace(b"\r\n", b"\n")).hexdigest()
                if digest != norm_sha:
                    return False, f"{sig_companion.name}: digest mismatch against {json_path.name}"
            if sig_hex and nonce:
                import watchmen_client  # type: ignore

                target_digest = digest or actual_sha
                res = watchmen_client.call_daemon(
                    "verify_signature",
                    {"nonce": nonce, "digest": target_digest, "signature": sig_hex},
                )
                if isinstance(res, dict) and res.get("success") is True and not res.get("error"):
                    return True, f"Verified detached JSON signature {sig_companion.name}"
                return False, f"{sig_companion.name}: Watchmen daemon rejected signature"
    except json.JSONDecodeError as exc:
        sys.stderr.write(
            f"[validate-story-completion-gate] WARN: {sig_companion.name} is not JSON ({exc}); trying legacy signature format\n"
        )

    if _verify_watchmen_signature(data, sig_text):
        return True, f"Verified detached Watchmen signature {sig_companion.name}"

    return False, f"{json_path.name}: cryptographic signature verification failed"


def parse_cli_args(argv: list[str]) -> Tuple[str, Path]:
    parser = argparse.ArgumentParser(description="Zero-Trust Read-Only Checkpoint Gate for Story Completion")
    parser.add_argument("--story-id", "--story", dest="story_id", required=False, help="ID of the story being completed")
    parser.add_argument("--root", default=str(PROJECT_ROOT), help="Project root directory")
    parser.add_argument("positionals", nargs="*", help="Optional positional [epic_id] <story_id>")
    args = parser.parse_args(argv)

    story_id = args.story_id
    if not story_id and args.positionals:
        story_id = args.positionals[-1]

    if not story_id:
        parser.error("Must provide --story-id <ID> (or positional <story_id>).")

    story_id = str(story_id).strip()
    if story_id.lower().startswith("story-"):
        story_id = story_id[6:]
    return story_id, Path(args.root).resolve()


def _policy_requires_sig_receipts(project_root: Path) -> bool:
    """Read-only O_NOFOLLOW check of .agent/config/gate-policy-matrix.yaml.
    Defaults to True (fail-closed) when missing/unreadable; returns False only when
    iWish v4.1 policy explicitly sets top-level require_sig_receipts: false."""
    policy_path = project_root / ".agent" / "config" / "gate-policy-matrix.yaml"
    raw = _safe_read_regular_bytes(policy_path)
    if raw is None:
        return True
    text = raw.decode("utf-8", errors="ignore")
    try:
        import yaml  # type: ignore

        parsed = yaml.safe_load(text)
        if isinstance(parsed, dict) and parsed.get("require_sig_receipts") is False:
            return False
        return True
    except Exception:
        if re.search(r"(?m)^require_sig_receipts\s*:\s*false\s*(?:#.*)?$", text):
            return False
        return True


def _verify_v41_deterministic_story_hygiene(story_id: str, project_root: Path) -> Tuple[bool, str]:
    """Lightweight iWish v4.1 structural hygiene check (O_NOFOLLOW, regular story.md, no 'done' status)."""
    story_md_candidates: list[Path] = []
    epic_story_root = project_root / "_iwish-output" / "3. Development" / "1. Epic & Story"
    if epic_story_root.exists():
        for p in epic_story_root.rglob(f"Story-{story_id}"):
            if p.is_dir() and not os.path.islink(str(p)):
                story_md_candidates.append(p / "story.md")
    flat_story = project_root / "_iwish-output" / "stories" / f"story-{story_id}.md"
    if os.path.lexists(str(flat_story)):
        story_md_candidates.append(flat_story)

    if not story_md_candidates:
        return False, f"Story-{story_id}/story.md not found"

    for md_path in story_md_candidates:
        raw = _safe_read_regular_bytes(md_path)
        if raw is None:
            return False, f"{md_path.name}: missing, 0-byte, symlink, or hardlink"
        text = raw.decode("utf-8", errors="ignore")
        if re.search(r"(?im)^status\s*:\s*['\"]?done['\"]?\s*$", text):
            return False, f"{md_path.name}: forbidden status 'done' (must be lowercase 'completed')"

    return True, f"iWish v4.1 deterministic hygiene passed for Story {story_id} (require_sig_receipts: false)"


def main(argv: Optional[list[str]] = None) -> int:
    story_id, project_root = parse_cli_args(sys.argv[1:] if argv is None else argv)

    # Input Sanitization for Path Traversal & Injection
    if not re.match(r"^[A-Za-z0-9_.\-]+$", story_id) or ".." in story_id:
        print("[FATAL] Invalid story_id format.", file=sys.stderr)
        return 1

    # 1. Check if the story is claiming to be completed in sprint-status.yaml or story.md
    if not _is_story_marked_completed(story_id, project_root):
        print(f"[GATE] Story {story_id} is not marked completed. Gate bypassed.")
        return 0

    # 1b. Honor iWish v4.1 policy (require_sig_receipts: false) when declared in gate-policy-matrix.yaml
    if not _policy_requires_sig_receipts(project_root):
        ok_v41, detail_v41 = _verify_v41_deterministic_story_hygiene(story_id, project_root)
        if ok_v41:
            print(f"[GATE] {detail_v41}.")
            return 0
        report_validation_failure(story_id, detail_v41)
        return 1

    # 2. Gather candidate cryptographic JSON evidence files (Read-Only)
    json_candidates: list[Path] = [
        project_root / "_iwish-output" / "reviews" / f"Story-{story_id}-review-evidence.json",
    ]
    epic_story_root = project_root / "_iwish-output" / "3. Development" / "1. Epic & Story"
    if epic_story_root.exists():
        try:
            for p in epic_story_root.rglob(f"Story-{story_id}"):
                if p.is_dir() and not os.path.islink(str(p)):
                    json_candidates.extend(
                        [
                            p / "qa-approved-evidence.json",
                            p / "qa" / "evidence" / "closeout-attestation.json",
                            p / "pipeline-evidence-review.json",
                            p / "pipeline-evidence-qa.json",
                            p / "qa" / "evidence" / "review-run-manifest.json",
                            p / "qa" / "evidence" / "ocr-findings.json",
                        ]
                    )
        except OSError as exc:
            sys.stderr.write(
                f"[validate-story-completion-gate] WARN: failed scanning {epic_story_root} for evidence of Story-{story_id}: {exc}\n"
            )

    flat_story_dir = project_root / "_iwish-output" / "stories"
    if flat_story_dir.exists():
        json_candidates.append(flat_story_dir / f"pipeline-evidence-review-{story_id}.json")

    rejection_reasons: list[str] = []
    for candidate in json_candidates:
        if os.path.lexists(str(candidate)):
            ok, detail = _verify_pipeline_or_review_json(candidate, story_id, project_root)
            if ok:
                print(f"[GATE] Zero-Trust Verification Passed for Story {story_id} ({detail}).")
                return 0
            rejection_reasons.append(detail)

    reason_summary = "; ".join(rejection_reasons) if rejection_reasons else (
        f"No cryptographically signed review/QA evidence found for Story {story_id} "
        "(0-byte files, symlinks, and unsigned markdown reports are strictly rejected)"
    )
    report_validation_failure(story_id, reason_summary)
    return 1


if __name__ == "__main__":
    sys.exit(main())
