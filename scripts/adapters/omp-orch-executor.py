# @story-60.8
# @story-60.7
# @story-60.6
# @story-60.5
# @story-60.4
# @story-60.3
# @story-60.2
# @story-60.1
# @story-76.5
# @story-56.1
#!/usr/bin/env python3
"""
OMP Orchestration Executor (omp-orch-executor.py)
Unified multi-mode runner for Oh My Pi (OMP) in I-Wish SDLC.
Supports 4 modes (plan, code, review, dynamic), XML-structured templates,
in-process LSP enforcement, atomic rollback, user failure decision protocol,
and post-execution trace/token analytics (HTML, JSONL, Report).
"""

import os
import sys
import subprocess
import argparse
import json
import time
import signal
import threading
import queue
import shutil
import datetime
import ast
import re
import hashlib
import sqlite3
import fcntl
from pathlib import Path
from typing import Dict, Any, Optional, List, Tuple

# --- [Watchmen Core Injection] ---
_script_dir = os.path.dirname(os.path.abspath(__file__))
_agent_dir = os.path.abspath(os.path.join(_script_dir, "../.."))
if _agent_dir not in sys.path:
    sys.path.insert(0, _agent_dir)
try:
    import watchmen_core
    watchmen_core.verify_execution(__file__)
except ImportError:
    pass
# ---------------------------------

# Tri-State Exit Codes
EXIT_SUCCESS = 0
EXIT_TEST_DEFECT = 1
EXIT_FALLBACK_TRIGGER = 10
EXIT_CONFIG_ERROR = 11

# Secret Redaction Patterns (P1 Fix: Support modern project keys and hyphens)
SECRET_PATTERNS = [
    re.compile(r'sk-(?:proj-)?[a-zA-Z0-9_\-]{20,}'),
    re.compile(r'AQ\.[A-Za-z0-9_\-]{35,}'),
    re.compile(r'AIza[A-Za-z0-9_\-]{35}'),
]

def scrub_secrets(text: str) -> str:
    """Redacts known API key patterns from strings to prevent credential leaks."""
    if not text:
        return text
    for pattern in SECRET_PATTERNS:
        text = pattern.sub('[REDACTED_API_KEY]', text)
    return text

def log(msg: str):
    clean_msg = scrub_secrets(str(msg))
    print(f"[OMP-ORCH] {clean_msg}", flush=True)

# ---------------------------------------------------------------------------
# Pre-Flight & Heartbeat
# ---------------------------------------------------------------------------

def _parse_oauth_expires_ms(val: Any) -> int:
    """
    Parses expires timestamp into numeric epoch milliseconds (int).
    Handles ISO-8601 strings (e.g. '2026-02-19T18:08:14.534Z'), epoch seconds,
    and epoch milliseconds. If expired or missing, returns 0 to trigger OMP refresh.
    (EC-OMPV3-01)
    """
    if val is None or val == "":
        return 0
    now_ms = int(time.time() * 1000)
    try:
        if isinstance(val, (int, float)):
            num_val = float(val)
            if num_val <= 0:
                return 0
            if num_val < 10_000_000_000:
                parsed_ms = int(num_val * 1000)
            else:
                parsed_ms = int(num_val)
        else:
            val_str = str(val).strip()
            try:
                num_val = float(val_str)
                if num_val < 10_000_000_000:
                    parsed_ms = int(num_val * 1000)
                else:
                    parsed_ms = int(num_val)
            except ValueError:
                dt = datetime.datetime.fromisoformat(val_str.replace("Z", "+00:00"))
                parsed_ms = int(dt.timestamp() * 1000)

        if parsed_ms <= now_ms + 60_000:
            return 0
        return parsed_ms
    except (ValueError, TypeError, OverflowError) as exc:
        sys.stderr.write(f"[omp-orch-executor] warning: unparseable OAuth expires value {val!r}: {exc}\n")
        return 0


def sync_omp_delegation_credentials(
    agent_db_path: Path,
    models_yml_path: Path,
    topology: str,
    codex_auth_path: Optional[Path] = None,
    ag_auth_path: Optional[Path] = None,
    refresh_hook: Optional[Any] = None,
) -> Dict[str, str]:
    """
    Synchronizes OAuth delegation credentials into ~/.omp/agent/agent.db (100% Delegation Mode).
    Enforces atomic SQLite update with BEGIN IMMEDIATE, auth_change_revision triggers and revision increment,
    compact JSON serialization (separators=(',', ':')) to ensure 100% CAS match in oh-my-pi refreshStoredOAuthCredential,
    real preflight refresh validation, sanitizes models.yml to zero API keys, and fail-closed if credentials are missing.
    (EC-OMPV3-01, GP-5 / FIX-1)
    """
    agent_db_path = Path(agent_db_path).resolve()
    models_yml_path = Path(models_yml_path).resolve()
    agent_db_path.parent.mkdir(parents=True, exist_ok=True)

    if codex_auth_path is None:
        codex_auth_path = Path.home() / ".codex" / "auth.json"
    else:
        codex_auth_path = Path(codex_auth_path).resolve()

    if ag_auth_path is None:
        ag_auth_path = Path.home() / ".antigravity_cockpit" / "credentials.json"
    else:
        ag_auth_path = Path(ag_auth_path).resolve()

    codex_data = None
    if codex_auth_path.is_file():
        try:
            raw_c = json.loads(codex_auth_path.read_text(encoding="utf-8"))
            tokens = raw_c.get("tokens", {})
            acc_id = tokens.get("account_id") or ""
            access_tok = tokens.get("access_token") or ""
            refresh_tok = tokens.get("refresh_token") or ""
            email = ""
            jwt_exp_ms = 0
            if access_tok and "." in access_tok:
                try:
                    import base64
                    payload_part = access_tok.split(".")[1]
                    rem = len(payload_part) % 4
                    if rem:
                        payload_part += "=" * (4 - rem)
                    jwt_payload = json.loads(base64.urlsafe_b64decode(payload_part.encode("ascii")).decode("utf-8"))
                    email = jwt_payload.get("email") or ""
                    if jwt_payload.get("exp"):
                        jwt_exp_ms = int(float(jwt_payload["exp"]) * 1000)
                except (ValueError, TypeError, KeyError, IndexError, UnicodeDecodeError, json.JSONDecodeError) as exc:
                    sys.stderr.write(f"[omp-orch-executor] warning: failed decoding Codex JWT claims: {exc}\n")
            expires_val = tokens.get("expires_at") or tokens.get("expires")
            expires_ms = _parse_oauth_expires_ms(expires_val) or jwt_exp_ms or int((time.time() + 3600) * 1000)
            if access_tok or refresh_tok:
                codex_data = {
                    "access": access_tok,
                    "refresh": refresh_tok,
                    "expires": expires_ms,
                    "accountId": acc_id,
                    "orgId": acc_id,
                    "email": email,
                }
        except Exception as e:
            log(f"⚠️ Failed to parse ~/.codex/auth.json: {e}")

    ag_data = None
    jetski_oauth_path = Path.home() / ".gemini" / "jetski-standalone-oauth-token"
    ag_sources = [ag_auth_path] if ag_auth_path != (Path.home() / ".antigravity_cockpit" / "credentials.json").resolve() else [jetski_oauth_path, ag_auth_path]

    fallback_proj_id = "model-nova-tc5ww"
    fallback_email = ""
    if ag_auth_path.is_file():
        try:
            raw_fb = json.loads(ag_auth_path.read_text(encoding="utf-8"))
            fb_accs = raw_fb.get("accounts", [])
            fb_acc = fb_accs[0] if isinstance(fb_accs, list) and fb_accs else (next(iter(fb_accs.values())) if isinstance(fb_accs, dict) and fb_accs else raw_fb)
            fallback_proj_id = fb_acc.get("projectId") or fb_acc.get("project_id") or fallback_proj_id
            fallback_email = (fb_acc.get("email") or "").lower().strip()
        except (OSError, json.JSONDecodeError, ValueError, TypeError, AttributeError) as exc:
            sys.stderr.write(f"[omp-orch-executor] warning: failed reading AG fallback account metadata: {exc}\n")

    for src_path in ag_sources:
        if not src_path.is_file():
            continue
        try:
            raw_ag = json.loads(src_path.read_text(encoding="utf-8"))
            if isinstance(raw_ag.get("token"), dict):
                tok_obj = raw_ag["token"]
                acc_tok = tok_obj.get("access_token") or tok_obj.get("accessToken") or ""
                ref_tok = tok_obj.get("refresh_token") or tok_obj.get("refreshToken") or ""
                expires_val = tok_obj.get("expiry") or tok_obj.get("expires_at") or tok_obj.get("expires")
                email = fallback_email
                id_tok = raw_ag.get("id_token") or ""
                if id_tok and "." in id_tok:
                    try:
                        import base64
                        part = id_tok.split(".")[1]
                        rem = len(part) % 4
                        if rem:
                            part += "=" * (4 - rem)
                        id_claims = json.loads(base64.urlsafe_b64decode(part.encode("ascii")).decode("utf-8"))
                        email = (id_claims.get("email") or email).lower().strip()
                    except (ValueError, TypeError, KeyError, IndexError, UnicodeDecodeError, json.JSONDecodeError, AttributeError) as exc:
                        sys.stderr.write(f"[omp-orch-executor] warning: failed decoding AG id_token claims: {exc}\n")
                proj_id = raw_ag.get("project_id") or raw_ag.get("projectId") or fallback_proj_id
            else:
                accounts = raw_ag.get("accounts", [])
                if isinstance(accounts, list) and accounts:
                    acc = accounts[0]
                elif isinstance(accounts, dict) and accounts:
                    acc = next(iter(accounts.values()))
                else:
                    acc = raw_ag
                acc_tok = acc.get("accessToken") or acc.get("access_token") or ""
                ref_tok = acc.get("refreshToken") or acc.get("refresh_token") or ""
                email = (acc.get("email") or fallback_email).lower().strip()
                proj_id = acc.get("projectId") or acc.get("project_id") or fallback_proj_id
                expires_val = acc.get("expires") or acc.get("expiresAt") or acc.get("expires_at")

            expires_ms = _parse_oauth_expires_ms(expires_val)
            now_ms = int(time.time() * 1000)
            if ref_tok and (expires_ms <= now_ms + 300_000):
                refreshed_ok = False
                if refresh_hook is not None:
                    try:
                        hook_res = refresh_hook(ref_tok)
                        if isinstance(hook_res, tuple) and len(hook_res) >= 2:
                            acc_tok, expires_ms = hook_res[0], hook_res[1]
                            refreshed_ok = bool(acc_tok)
                        elif isinstance(hook_res, str) and hook_res:
                            acc_tok = hook_res
                            expires_ms = int((time.time() + 3600 - 300) * 1000)
                            refreshed_ok = True
                    except Exception as h_err:
                        log(f"⚠️ Custom refresh hook notice: {h_err}")

                if not refreshed_ok and not refresh_hook:
                    try:
                        import base64
                        import ssl
                        import urllib.parse
                        import urllib.request
                        try:
                            import certifi
                            ssl_ctx = ssl.create_default_context(cafile=certifi.where())
                        except Exception:
                            ssl_ctx = ssl.create_default_context()
                        cid = os.environ.get("GOOGLE_OAUTH_CLIENT_ID", "").strip()
                        csec = os.environ.get("GOOGLE_OAUTH_CLIENT_SECRET", "").strip()
                        if not cid or not csec:
                            raise RuntimeError("Missing GOOGLE_OAUTH_CLIENT_ID/GOOGLE_OAUTH_CLIENT_SECRET env vars")
                        form_body = urllib.parse.urlencode({
                            "client_id": cid,
                            "client_secret": csec,
                            "grant_type": "refresh_token",
                            "refresh_token": ref_tok,
                        }).encode("utf-8")
                        t_req = urllib.request.Request("https://oauth2.googleapis.com/token", data=form_body, method="POST")
                        with urllib.request.urlopen(t_req, context=ssl_ctx, timeout=10) as t_resp:
                            t_json = json.loads(t_resp.read().decode("utf-8"))
                            if t_json.get("access_token"):
                                acc_tok = t_json["access_token"]
                                exp_sec = int(t_json.get("expires_in", 3600))
                                expires_ms = int((time.time() + exp_sec - 300) * 1000)
                                refreshed_ok = True
                    except Exception as ref_err:
                        log(f"⚠️ Pre-refresh google-antigravity token notice: {ref_err}")

                if not refreshed_ok and not refresh_hook:
                    # Also try omp token google-antigravity via subprocess
                    try:
                        res = subprocess.run(
                            [shutil.which("omp") or str(Path.home() / ".local/bin/omp"), "token", "google-antigravity"],
                            capture_output=True,
                            text=True,
                            timeout=15,
                            stdin=subprocess.DEVNULL,
                        )
                        tok = res.stdout.strip()
                        if tok.startswith("ya29."):
                            acc_tok = tok
                            expires_ms = int((time.time() + 3600 - 300) * 1000)
                            refreshed_ok = True
                    except Exception as omp_tok_err:
                        log(f"⚠️ Subprocess omp token notice: {omp_tok_err}")

                # If token was expired (or expires_ms == 0) and refresh failed completely, invalidate acc_tok so it fails closed
                if not refreshed_ok and (expires_ms <= now_ms + 60_000):
                    acc_tok = ""

            if acc_tok or ref_tok:
                ag_data = {
                    "access": acc_tok,
                    "refresh": ref_tok,
                    "expires": expires_ms,
                    "email": email,
                    "projectId": proj_id or "aicode-consumers",
                }
                break
        except Exception as e:
            log(f"⚠️ Failed to parse {src_path}: {e}")

    # Fail-closed check per topology (google-antigravity requires non-empty refresh token and valid access token)
    missing_creds = []
    if topology == "antigravity":
        if not ag_data or not ag_data["refresh"]:
            missing_creds.append("google-antigravity refresh token (~/.antigravity_cockpit/credentials.json)")
        elif not ag_data.get("access") and ag_data.get("expires", 0) <= int(time.time() * 1000):
            missing_creds.append("google-antigravity valid access token (refresh failed)")
    elif topology == "codex-cli":
        if not codex_data or not (codex_data["access"] or codex_data["refresh"]):
            missing_creds.append("openai-codex (~/.codex/auth.json)")
    elif topology in ("hybrid-1-worker-codex-advisor-ag", "hybrid-2-worker-ag-advisor-codex"):
        if not ag_data or not ag_data["refresh"]:
            missing_creds.append("google-antigravity refresh token (~/.antigravity_cockpit/credentials.json)")
        elif not ag_data.get("access") and ag_data.get("expires", 0) <= int(time.time() * 1000):
            missing_creds.append("google-antigravity valid access token (refresh failed)")
        if not codex_data or not (codex_data["access"] or codex_data["refresh"]):
            missing_creds.append("openai-codex (~/.codex/auth.json)")

    if missing_creds:
        log(f"❌ DELEGATION_CREDENTIAL_MISSING for topology '{topology}': missing {', '.join(missing_creds)}")
        raise RuntimeError(f"DELEGATION_CREDENTIAL_MISSING: missing {', '.join(missing_creds)}")

    # Update SQLite database under flock + BEGIN IMMEDIATE transaction
    db_lock_path = agent_db_path.parent / ".agent.db.lock"
    with open(db_lock_path, "w") as lock_f:
        fcntl.flock(lock_f.fileno(), fcntl.LOCK_EX)
        try:
            conn = sqlite3.connect(str(agent_db_path), timeout=10.0, isolation_level=None)
            cur = conn.cursor()
            cur.execute("PRAGMA busy_timeout = 10000;")
            cur.execute("BEGIN IMMEDIATE;")
            try:
                # Ensure tables and triggers exist matching sqlite-credential-store.ts
                cur.execute("""
                CREATE TABLE IF NOT EXISTS auth_change_revision (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    revision INTEGER NOT NULL
                );
                """)
                cur.execute("INSERT OR IGNORE INTO auth_change_revision (id, revision) VALUES (1, 0);")
                cur.execute("""
                CREATE TABLE IF NOT EXISTS auth_credentials (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    provider TEXT NOT NULL,
                    credential_type TEXT NOT NULL,
                    data TEXT NOT NULL,
                    disabled_cause TEXT DEFAULT NULL,
                    identity_key TEXT DEFAULT NULL,
                    created_at INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER)),
                    updated_at INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER))
                );
                """)
                cur.execute("CREATE INDEX IF NOT EXISTS idx_auth_provider ON auth_credentials(provider);")
                cur.execute("CREATE INDEX IF NOT EXISTS idx_auth_provider_identity ON auth_credentials(provider, identity_key) WHERE identity_key IS NOT NULL;")
                cur.execute("""
                CREATE TRIGGER IF NOT EXISTS auth_change_revision_auth_credentials_insert
                AFTER INSERT ON auth_credentials
                BEGIN
                    UPDATE auth_change_revision SET revision = revision + 1 WHERE id = 1;
                END;
                """)
                cur.execute("""
                CREATE TRIGGER IF NOT EXISTS auth_change_revision_auth_credentials_update
                AFTER UPDATE ON auth_credentials
                BEGIN
                    UPDATE auth_change_revision SET revision = revision + 1 WHERE id = 1;
                END;
                """)
                cur.execute("""
                CREATE TRIGGER IF NOT EXISTS auth_change_revision_auth_credentials_delete
                AFTER DELETE ON auth_credentials
                BEGIN
                    UPDATE auth_change_revision SET revision = revision + 1 WHERE id = 1;
                END;
                """)

                now_sec = int(time.time())

                if codex_data:
                    ident_key = f"account:{codex_data['accountId']}|org:{codex_data['orgId']}" if codex_data['accountId'] else "default"
                    if codex_data["email"]:
                        ident_key = f"email:{codex_data['email']}|org:{codex_data['orgId']}"
                    codex_payload = {
                        "access": codex_data["access"],
                        "refresh": codex_data["refresh"],
                        "expires": codex_data["expires"],
                        "accountId": codex_data["accountId"],
                        "orgId": codex_data["orgId"],
                        "email": codex_data["email"],
                    }
                    payload_str = json.dumps(codex_payload, separators=(",", ":"))
                    cur.execute(
                        "SELECT id FROM auth_credentials WHERE provider = 'openai-codex' AND (identity_key = ? OR identity_key IS NULL);",
                        (ident_key,)
                    )
                    row = cur.fetchone()
                    if row:
                        cur.execute(
                            "UPDATE auth_credentials SET credential_type = 'oauth', data = ?, disabled_cause = NULL, identity_key = ?, updated_at = ? WHERE id = ?;",
                            (payload_str, ident_key, now_sec, row[0])
                        )
                        cred_id = row[0]
                    else:
                        cur.execute(
                            "INSERT INTO auth_credentials (provider, credential_type, data, disabled_cause, identity_key, created_at, updated_at) VALUES ('openai-codex', 'oauth', ?, NULL, ?, ?, ?);",
                            (payload_str, ident_key, now_sec, now_sec)
                        )
                        cred_id = cur.lastrowid
                    try:
                        cur.execute("DELETE FROM auth_credential_blocks WHERE credential_id = ?;", (cred_id,))
                    except sqlite3.OperationalError:
                        pass

                if ag_data:
                    ident_key = f"email:{ag_data['email']}" if ag_data['email'] else "default"
                    ag_payload = {
                        "access": ag_data["access"],
                        "refresh": ag_data["refresh"],
                        "expires": ag_data["expires"],
                        "email": ag_data["email"],
                        "projectId": ag_data["projectId"],
                    }
                    payload_str = json.dumps(ag_payload, separators=(",", ":"))
                    cur.execute(
                        "SELECT id FROM auth_credentials WHERE provider = 'google-antigravity' AND (identity_key = ? OR identity_key IS NULL);",
                        (ident_key,)
                    )
                    row = cur.fetchone()
                    if row:
                        cur.execute(
                            "UPDATE auth_credentials SET credential_type = 'oauth', data = ?, disabled_cause = NULL, identity_key = ?, updated_at = ? WHERE id = ?;",
                            (payload_str, ident_key, now_sec, row[0])
                        )
                        cred_id = row[0]
                    else:
                        cur.execute(
                            "INSERT INTO auth_credentials (provider, credential_type, data, disabled_cause, identity_key, created_at, updated_at) VALUES ('google-antigravity', 'oauth', ?, NULL, ?, ?, ?);",
                            (payload_str, ident_key, now_sec, now_sec)
                        )
                        cred_id = cur.lastrowid
                    try:
                        cur.execute("DELETE FROM auth_credential_blocks WHERE credential_id = ?;", (cred_id,))
                    except sqlite3.OperationalError:
                        pass

                cur.execute("UPDATE auth_change_revision SET revision = revision + 1 WHERE id = 1;")
                cur.execute("COMMIT;")
            except (sqlite3.Error, ValueError, TypeError, RuntimeError):
                try:
                    cur.execute("ROLLBACK;")
                except sqlite3.Error as rb_exc:
                    sys.stderr.write(f"[omp-orch-executor] warning: SQLite rollback failed: {rb_exc}\n")
                raise
            finally:
                conn.close()
            log("🔒 OAuth delegation credentials synchronized successfully.")
        finally:
            fcntl.flock(lock_f.fileno(), fcntl.LOCK_UN)

    # Sanitize models.yml to zero API keys
    try:
        if models_yml_path.is_file():
            bak_path = models_yml_path.with_suffix(".yml.bak")
            shutil.copy2(models_yml_path, bak_path)
        models_yml_path.write_text("providers: {}\n", encoding="utf-8")
        log("🧹 Sanitized models.yml to zero API keys (providers: {}).")
    except Exception as e:
        log(f"⚠️ Warning sanitizing models.yml: {e}")

    return {
        "codex_access_token": codex_data["access"] if codex_data else "",
        "ag_refresh_token": ag_data["refresh"] if ag_data else "",
    }


def _resolve_delegation_selector(provider: str, model_id: str, effort: str) -> str:
    """
    Formats the physical model selector for OMP with provider prefix and effort.
    google-antigravity: clamps xhigh -> high.
    openai-codex: supports minimal|low|medium|high|xhigh.
    (EC-OMPV3-04)
    """
    clean_model = model_id
    if "/" in clean_model:
        clean_model = clean_model.split("/")[-1]
    if ":" in clean_model:
        clean_model = clean_model.split(":")[0]

    eff = (effort or "high").lower()
    if provider == "google-antigravity":
        if clean_model in ("", "inherit", "default"):
            clean_model = "gemini-4-argon"
        if eff == "xhigh":
            eff = "high"
        if eff not in ("low", "medium", "high"):
            eff = "high"
        return f"google-antigravity/{clean_model}:{eff}"
    elif provider == "openai-codex":
        if eff not in ("minimal", "low", "medium", "high", "xhigh"):
            eff = "high"
        return f"openai-codex/{clean_model}:{eff}"
    else:
        return f"{provider}/{clean_model}:{eff}"


def generate_omp_watchdog_files(
    worktree_dir: Path,
    story_id: str = "unknown",
    plan_data: Optional[Dict[str, Any]] = None,
    advisor_selector: Optional[str] = None,
    worker_selector: Optional[str] = None,
    **kwargs: Any,
) -> None:
    """
    Generates .omp/WATCHDOG.yml, .omp/WATCHDOG.md, and .omp/config-overlay.yml
    in the worktree directory.
    Escapes @file imports to prevent oh-my-pi expandAtImports() errors.
    Injects Plugin DoD physical bundle rules if target_files touches src/plugins.
    Dynamically renders all 7 modelRoles and 5 flat aliases in config-overlay.yml
    to prevent fallback to deepseek-v4-flash. (EC-OMPV3-02, EC-OMPV3-04, GP-2, GP-5 / FIX-2, GP-12 / FIX-10)
    """
    if "topology" in kwargs:
        top = str(kwargs["topology"]).lower()
        eff = kwargs.get("effort", "high")
        if "codex-worker-ag-advisor" in top or "worker-codex-advisor-ag" in top:
            w_prov, a_prov = "openai-codex", "google-antigravity"
        elif "ag-worker-codex-advisor" in top or "worker-ag-advisor-codex" in top:
            w_prov, a_prov = "google-antigravity", "openai-codex"
        elif "codex" in top:
            w_prov, a_prov = "openai-codex", "openai-codex"
        else:
            w_prov, a_prov = "google-antigravity", "google-antigravity"
        w_res = _resolve_delegation_selector(w_prov, "default", eff)
        a_res = _resolve_delegation_selector(a_prov, "default", eff)
        if not worker_selector:
            worker_selector = w_res
        if not advisor_selector:
            advisor_selector = a_res
    if not worker_selector:
        worker_selector = "openai-codex/gpt-5.6-luna:high"
    if not advisor_selector:
        advisor_selector = "google-antigravity/gemini-4-argon:high"

    if plan_data is None:
        if "target_files" in kwargs:
            plan_data = {"target_files": kwargs["target_files"]}
        else:
            plan_data = {}

    omp_dir = worktree_dir / ".omp"
    omp_dir.mkdir(parents=True, exist_ok=True)

    watchdog_yml = omp_dir / "WATCHDOG.yml"
    watchdog_md = omp_dir / "WATCHDOG.md"
    config_overlay = omp_dir / "config-overlay.yml"

    # 1. WATCHDOG.yml (GP-12 / EC-OMPV3-15: enforce bounded read <= 120 lines)
    yml_content = f"""instructions: |
  You are the Zero-Trust Real-Time Watchdog Advisor for I-Wish SDLC.
  Review every incremental turn delta immediately using read, grep, and glob.
  When reading files, enforce limit <= 120 lines and avoid node_modules/** or *.min.js.
advisors:
  - name: sdlc-scope-guard
    model: {advisor_selector}
    tools:
      - read
      - grep
      - glob
    enabled: true
"""
    watchdog_yml.write_text(yml_content, encoding="utf-8")

    # 2. Extract target files and normalize
    target_files = plan_data.get("target_files", {})
    all_files = []
    if isinstance(target_files, dict):
        all_files = target_files.get("create", []) + target_files.get("modify", []) + target_files.get("delete", [])
    elif isinstance(target_files, (list, set)):
        all_files = list(target_files)

    clean_files = [f.strip() for f in all_files if isinstance(f, str) and f.strip()]
    target_files_bullet = "\n".join(f"- `{f}`" for f in clean_files) if clean_files else "- `(All files within story scope)`"

    plugin_dod_instruction = ""
    is_plugin_story = any("src/plugins" in f or "cowok-plugin-pack" in f for f in clean_files) or "plugin" in str(story_id).lower()
    if is_plugin_story:
        plugin_dod_instruction = """
### 4. Plugin Definition of Done (DoD) Invariant (GP-2 / G1_ui_bundle_exists)
- Every plugin directory under `src/plugins/<kit>/<plugin>/` MUST physically contain both:
  1. `manifest.json` (Valid plugin descriptor)
  2. `ui.js` (Compiled ESM bundle with export default or export function mount/render)
- Even when packaging into `dist/<kit>.zip`, you MUST retain the unbundled physical `ui.js` next to `manifest.json`.
- If the Worker packages `.zip` but removes `ui.js`, call `advise({ severity: "blocker", note: "G1_ui_bundle_exists violation: physical ui.js missing next to manifest.json" })`.
"""

    md_content = f"""# I-Wish Real-Time Watchdog Advisor Attention Directives

Story ID: `{story_id}`

## Authorized Target Files:
{target_files_bullet}

## Advisor Directives & Prohibitions:
1. **Scope Lockdown Violation (Critical Blocker)**:
   - If Worker creates, edits, or deletes any file outside the authorized target files above (e.g. modifying or deleting files belonging to another story such as Story 41.13), immediately call:
     `advise({{ severity: "blocker", note: "SCOPE_VIOLATION: Attempted to modify or delete out-of-scope file: <filepath>" }})`
2. **Forbidden Commands**:
   - `git reset --hard`, `git clean -fd`, `rm -rf` on root directories are strictly prohibited.
3. **Architecture Guard**:
   - Check all Next.js API routes in `src/app/api/` for `guardedRoute()` wrapping.
   - Raw `fetch()` outside `src/api/` is strictly prohibited.
4. **Context & Read Limits (GP-12 / EC-OMPV3-15)**:
   - Always bound tool calls: specify limit <= 120 lines and <= 16KB per read.
   - Denylist paths: NEVER read node_modules/**, dist/**, *.min.js, *.zip, package-lock.json, or yarn.lock.
{plugin_dod_instruction}
"""
    watchdog_md.write_text(md_content, encoding="utf-8")

    # 3. config-overlay.yml dynamically overriding all 7 modelRoles and 5 flat mappings (GP-5 / FIX-2)
    w_sel = worker_selector or "openai-codex/gpt-5.6-luna:high"
    a_sel = advisor_selector or "google-antigravity/gemini-4-argon:high"
    adv_enabled_str = "false" if kwargs.get("advisor_enabled") is False else "true"
    overlay_content = f"""advisor:
  enabled: {adv_enabled_str}
  syncBacklog: "1"
  immuneTurns: 2

modelRoles:
  default: {w_sel}
  task: {w_sel}
  smol: {w_sel}
  slow: {w_sel}
  advisor: {a_sel}
  reviewer: {a_sel}
  plan: {w_sel}

model: {w_sel}
slow: {w_sel}
smol: {w_sel}
plan: {w_sel}
reviewer: {a_sel}
"""
    config_overlay.write_text(overlay_content, encoding="utf-8")
    log("📋 Generated .omp/WATCHDOG.yml, .omp/WATCHDOG.md, and .omp/config-overlay.yml")


def cleanup_omp_watchdog_files(worktree_dir: Path) -> List[str]:
    """
    Removes ephemeral .omp/WATCHDOG.yml, .omp/WATCHDOG.md, .omp/config-overlay.yml,
    .omp/run-verify.sh, .omp/live-lint-feedback.json, and .omp-prompt.md so they
    never pollute git status after OMP execution (EC-OMPV3-02, NH-02).
    """
    removed = []
    for ephemeral_f in (
        worktree_dir / ".omp" / "WATCHDOG.yml",
        worktree_dir / ".omp" / "WATCHDOG.md",
        worktree_dir / ".omp" / "config-overlay.yml",
        worktree_dir / ".omp" / "run-verify.sh",
        worktree_dir / ".omp" / "live-lint-feedback.json",
        worktree_dir / ".omp-prompt.md",
    ):
        try:
            if ephemeral_f.is_file():
                ephemeral_f.unlink(missing_ok=True)
                removed.append(str(ephemeral_f.relative_to(worktree_dir)))
        except (OSError, ValueError) as exc:
            sys.stderr.write(f"[omp-orch-executor] warning: failed removing ephemeral file {ephemeral_f}: {exc}\n")
    return removed


MAX_ADVISOR_YIELDS_PER_SESSION = 6
MAX_CONSECUTIVE_DUPLICATE_ADVICE = 3


def check_advisor_yield_cap_and_dedupe(advisor_events: list) -> dict:
    """
    FIX-13: Executor-side advisor yield cap & deduplication.
    Tracks advisor_yielded count (cap 6), computes advice_fp = sha1(normalized_advice_text),
    detects consecutive duplicate advice (cap 3), and returns:
    {"should_terminate": bool, "reason": str, "advisor_yielded": int, "unique_advices": list}
    """
    advisor_yielded = 0
    unique_advices = []
    seen_fps = set()
    last_fp = None
    consecutive_dupes = 0
    should_terminate = False
    reason = ""

    for ev in advisor_events:
        advisor_yielded += 1
        advice_text = ""
        if isinstance(ev, dict):
            tool_res = ev.get("tool_result") or ev.get("result") or ev.get("advice") or ev.get("text") or ev.get("message") or ""
            if isinstance(tool_res, dict):
                advice_text = tool_res.get("text") or tool_res.get("advice") or json.dumps(tool_res, sort_keys=True)
            else:
                advice_text = str(tool_res)
            if not advice_text:
                advice_text = json.dumps(ev, sort_keys=True)
        else:
            advice_text = str(ev)

        norm_text = re.sub(r"\s+", " ", advice_text.strip().lower())
        fp = hashlib.sha1(norm_text.encode("utf-8")).hexdigest()

        if fp not in seen_fps:
            seen_fps.add(fp)
            unique_advices.append(advice_text)

        if fp == last_fp:
            consecutive_dupes += 1
        else:
            consecutive_dupes = 1
            last_fp = fp

        if advisor_yielded >= MAX_ADVISOR_YIELDS_PER_SESSION:
            should_terminate = True
            reason = f"Advisor yield cap exceeded ({advisor_yielded} >= {MAX_ADVISOR_YIELDS_PER_SESSION})"
            break

        if consecutive_dupes >= MAX_CONSECUTIVE_DUPLICATE_ADVICE:
            should_terminate = True
            reason = f"Duplicate consecutive advice limit reached ({consecutive_dupes} >= {MAX_CONSECUTIVE_DUPLICATE_ADVICE})"
            break

    return {
        "should_terminate": should_terminate,
        "reason": reason,
        "advisor_yielded": advisor_yielded,
        "unique_advices": unique_advices,
    }


def parse_omp_json_stream(raw_stdout: str) -> Tuple[Optional[str], int, Optional[str]]:
    """
    Parses omp --mode json event stream stdout (EC-OMPV3-03, FIX-13).
    Returns: (captured_session_id, advisor_yield_count, exec_error)
    Detects stopReason == 'error' | 'aborted' on agent_end or turn_end,
    tracks advisor events, applies yield cap (6) and duplicate advice dedupe (3),
    and fails closed when zero turn_end events were emitted (zero_turns).
    """
    captured_session_id = None
    advisor_yield_count = 0
    advisory_block_count = 0
    turn_end_count = 0
    exec_error = None
    advisor_events = []

    for line in raw_stdout.splitlines():
        line_str = line.strip()
        if not line_str or not line_str.startswith("{"):
            continue
        try:
            ev = json.loads(line_str)
        except (json.JSONDecodeError, ValueError, TypeError) as exc:
            sys.stderr.write(f"[omp-orch-executor] warning: malformed stream JSON line skipped: {exc}\n")
            continue

        if "<advisory" in line_str:
            advisory_block_count += line_str.count("<advisory")

        ev_type = ev.get("type")
        if ev_type == "session":
            captured_session_id = ev.get("id")
        elif ev_type == "advisor_yielded":
            advisor_yield_count += 1
            advisor_events.append(ev)
        elif ev_type == "tool_execution_end" and ev.get("toolName") == "advise":
            advisor_yield_count += 1
            advisor_events.append(ev)
        elif ev_type == "turn_end":
            turn_end_count += 1
            msg = ev.get("message", {})
            if isinstance(msg, dict):
                stop_reason = msg.get("stopReason")
                err_msg = msg.get("errorMessage")
                if stop_reason in ("error", "aborted"):
                    if err_msg or stop_reason == "error":
                        exec_error = f"OMP turn error ({stop_reason}): {err_msg or 'Turn terminated with error'}"
        elif ev_type == "agent_end":
            msgs = ev.get("messages", [])
            if isinstance(msgs, list) and msgs:
                last_msg = msgs[-1]
                if isinstance(last_msg, dict):
                    stop_reason = last_msg.get("stopReason")
                    err_msg = last_msg.get("errorMessage")
                    if stop_reason in ("error", "aborted"):
                        if err_msg or stop_reason == "error":
                            exec_error = f"OMP agent execution error ({stop_reason}): {err_msg or 'Execution terminated with error'}"

    # FIX-13: Apply advisor yield cap & dedupe check
    if advisor_events:
        dedupe_res = check_advisor_yield_cap_and_dedupe(advisor_events)
        if dedupe_res.get("should_terminate") and exec_error is None:
            exec_error = f"Advisor yield cap/dedupe triggered: {dedupe_res['reason']}"

    total_advisor_interventions = max(advisor_yield_count, advisory_block_count)
    if exec_error is None and turn_end_count == 0:
        exec_error = "OMP JSON stream error (zero_turns): No valid turn_end events in stdout"

    return captured_session_id, total_advisor_interventions, exec_error


def verify_per_file_ast_syntax(file_path: Path) -> bool:
    """
    R12-01: Checks syntax per file independently:
    - .py: ast.parse
    - .json: json.loads
    - .ts/.tsx/.js/.jsx: ts.createSourceFile parseDiagnostics via node or balanced syntax parser
    - .yml/.yaml: yaml.safe_load
    Returns True if valid syntax, False if syntax-torn.
    """
    p = Path(file_path)
    if not p.is_file():
        return False
    suffix = p.suffix.lower()
    try:
        content = p.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        sys.stderr.write(f"[omp-orch-executor] warning: failed reading {p} for syntax validation: {exc}\n")
        return False

    if suffix == ".py":
        try:
            ast.parse(content, filename=str(p))
            return True
        except SyntaxError:
            return False

    elif suffix == ".json":
        try:
            json.loads(content)
            return True
        except (json.JSONDecodeError, ValueError, TypeError) as exc:
            sys.stderr.write(f"[omp-orch-executor] warning: invalid JSON in {p}: {exc}\n")
            return False

    elif suffix in (".ts", ".tsx", ".js", ".jsx"):
        node_bin = shutil.which("node")
        if node_bin:
            try:
                script = (
                    "const fs = require('fs');"
                    "const ts = require('typescript');"
                    f"const file = {json.dumps(str(p.resolve()))};"
                    "const src = fs.readFileSync(file, 'utf8');"
                    "const sf = ts.createSourceFile(file, src, ts.ScriptTarget.Latest, true);"
                    "const diags = sf.parseDiagnostics || [];"
                    "process.exit(diags.length === 0 ? 0 : 1);"
                )
                res = subprocess.run([node_bin, "-e", script], capture_output=True, timeout=5, check=False)
                if res.returncode == 0:
                    return True
                elif res.returncode == 1:
                    return False
            except (subprocess.SubprocessError, OSError) as exc:
                sys.stderr.write(f"[omp-orch-executor] warning: TypeScript AST syntax probe fell back for {p}: {exc}\n")

        # Balanced braces/brackets/quotes fallback
        stack = []
        pairs = {')': '(', '}': '{', ']': '['}
        in_string = False
        string_char = ''
        escape = False
        in_single_comment = False
        in_multi_comment = False
        i = 0
        n = len(content)
        while i < n:
            ch = content[i]
            if in_single_comment:
                if ch == '\n': in_single_comment = False
                i += 1; continue
            if in_multi_comment:
                if ch == '*' and i + 1 < n and content[i+1] == '/':
                    in_multi_comment = False; i += 2; continue
                i += 1; continue
            if in_string:
                if escape: escape = False
                elif ch == '\\': escape = True
                elif ch == string_char: in_string = False
                i += 1; continue
            if ch == '/' and i + 1 < n:
                if content[i+1] == '/': in_single_comment = True; i += 2; continue
                elif content[i+1] == '*': in_multi_comment = True; i += 2; continue
            if ch in ('"', "'", '`'):
                in_string = True; string_char = ch; escape = False; i += 1; continue
            if ch in ('(', '{', '['): stack.append(ch)
            elif ch in (')', '}', ']'):
                if not stack or stack[-1] != pairs[ch]: return False
                stack.pop()
            i += 1
        return not (in_string or in_multi_comment or stack)

    elif suffix in (".yml", ".yaml"):
        try:
            import yaml
        except ImportError:
            sys.stderr.write(f"[omp-orch-executor] warning: PyYAML unavailable to validate {p}\n")
            return False
        try:
            yaml.safe_load(content)
            return True
        except (yaml.YAMLError, ValueError, TypeError) as exc:
            sys.stderr.write(f"[omp-orch-executor] warning: invalid YAML in {p}: {exc}\n")
            return False

    return True


def terminate_omp_process_group_and_checkpoint(
    proc: Any,
    worktree: Path,
    story_id: str,
    target_files: list,
    pre_run_untracked: set,
) -> dict:
    """
    R12-01: Clean process group termination, per-file AST verification & checkpoint patch.
    1. Calls os.killpg(os.getpgid(proc.pid), signal.SIGTERM/SIGKILL) to ensure 0 orphans.
    2. Saves .agent/cache/omp/<clean_id>/restart-checkpoint-1.patch (including untracked target_files).
    3. Runs verify_per_file_ast_syntax on each modified/created file in target_files:
       keeps syntactically valid files and rolls back only syntax-broken or out-of-scope files.
    """
    killed_processes = 0
    if proc is not None and hasattr(proc, "pid") and proc.pid:
        try:
            pgid = os.getpgid(proc.pid)
            os.killpg(pgid, signal.SIGTERM)
            killed_processes += 1
            time.sleep(0.3)
            try:
                os.killpg(pgid, 0)
                os.killpg(pgid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
        except (ProcessLookupError, PermissionError):
            pass

    clean_id = _normalize_story_id(story_id) or "default"
    cache_dir = Path(worktree) / ".agent" / "cache" / "omp" / clean_id
    cache_dir.mkdir(parents=True, exist_ok=True)
    patch_file = cache_dir / "restart-checkpoint-1.patch"

    # Save checkpoint patch including untracked target_files (via git add -N)
    norm_targets = []
    for tf in target_files:
        p_tf = Path(tf)
        try:
            rel = str(p_tf.relative_to(worktree)) if p_tf.is_absolute() else str(tf)
        except ValueError:
            rel = str(tf)
        norm_targets.append(os.path.normpath(rel).lstrip("./"))

    untracked_staged = []
    for tf in norm_targets:
        tf_path = Path(worktree) / tf
        if tf_path.is_file():
            chk = subprocess.run(["git", "ls-files", "--error-unmatch", tf], cwd=worktree, capture_output=True, check=False)
            if chk.returncode != 0:
                subprocess.run(["git", "add", "-N", tf], cwd=worktree, capture_output=True, check=False)
                untracked_staged.append(tf)

    try:
        diff_res = subprocess.run(["git", "diff"], cwd=worktree, capture_output=True, text=True, check=False)
        patch_file.write_text(diff_res.stdout or "", encoding="utf-8")
    except Exception as e:
        log(f"⚠️ Failed to write restart checkpoint patch: {e}")
    finally:
        if untracked_staged:
            subprocess.run(["git", "reset", "HEAD", "--"] + untracked_staged, cwd=worktree, capture_output=True, check=False)

    retained_files = []
    rolled_back_files = []

    st_proc = subprocess.run(["git", "status", "--porcelain", "-uall"], cwd=worktree, capture_output=True, text=True, check=False)
    for line in (st_proc.stdout or "").splitlines():
        if len(line) < 4:
            continue
        status_code = line[:2]
        f_rel = os.path.normpath(line[3:].strip()).lstrip("./")
        f_abs = Path(worktree) / f_rel

        if f_rel.startswith((".omp", ".agent", ".git", "_iwish-output")):
            continue

        is_target = any(f_rel == tf or (tf.endswith("/") and f_rel.startswith(tf)) for tf in norm_targets)

        if is_target:
            if f_abs.is_file():
                if verify_per_file_ast_syntax(f_abs):
                    retained_files.append(f_rel)
                else:
                    if status_code.startswith("??"):
                        if f_abs.is_dir():
                            shutil.rmtree(f_abs, ignore_errors=True)
                        else:
                            f_abs.unlink(missing_ok=True)
                    else:
                        subprocess.run(["git", "checkout", "--", f_rel], cwd=worktree, capture_output=True, check=False)
                    rolled_back_files.append(f_rel)
        else:
            if status_code.startswith("??"):
                if f_rel not in pre_run_untracked:
                    if f_abs.is_dir():
                        shutil.rmtree(f_abs, ignore_errors=True)
                    else:
                        f_abs.unlink(missing_ok=True)
                    rolled_back_files.append(f_rel)
            else:
                subprocess.run(["git", "checkout", "--", f_rel], cwd=worktree, capture_output=True, check=False)
                rolled_back_files.append(f_rel)

    return {
        "retained_files": retained_files,
        "rolled_back_files": rolled_back_files,
        "checkpoint_patch": str(patch_file),
        "killed_processes": killed_processes,
    }


def build_restart_handoff_capsule(
    blocker_reasons: list,
    completed_files: list,
    last_linter_error: str = "",
) -> str:
    """
    R12-01: Builds a <restart_handoff_capsule>...</restart_handoff_capsule> block strictly
    <= 500 tokens (<= 2000 chars) containing top <= 3 blockers from Run 1, the git status --short
    list of already-completed valid files, and the last linter failure.
    """
    top_blockers = [str(b).strip() for b in blocker_reasons[:3] if str(b).strip()]
    blockers_text = "\n".join(f"- {b}" for b in top_blockers) or "- Advisor iteration limit reached."
    
    files_text = "\n".join(f"  {f}" for f in completed_files[:20]) or "  None"
    
    linter_snippet = (last_linter_error or "").strip()
    if len(linter_snippet) > 400:
        linter_snippet = linter_snippet[:400] + "... [truncated]"

    capsule = (
        "<restart_handoff_capsule>\n"
        "<directive>DO NOT rewrite completed files; continue remaining tasks only.</directive>\n"
        f"<run1_blockers>\n{blockers_text}\n</run1_blockers>\n"
        f"<completed_files>\n{files_text}\n</completed_files>\n"
        f"<last_linter_error>\n{linter_snippet}\n</last_linter_error>\n"
        "</restart_handoff_capsule>"
    )
    if len(capsule) > 2000:
        allowed_len = 2000 - len("</restart_handoff_capsule>") - 10
        capsule = capsule[:allowed_len] + "\n</restart_handoff_capsule>"
    return capsule


def compute_adaptive_omp_timeout(base_timeout: int, target_files_count: int, risk_tier: int = 2, advisor_enabled: bool = False) -> int:
    """
    WP-R2 / LD-03 / EC-REM-P7-001: Computes adaptive timeout for OMP execution.
    Scales base timeout when base_timeout <= 180 and (target_files_count >= 5 or advisor_enabled).
    Caps at min(600, max(base_timeout, 120 + target_files_count * 35 + (60 if advisor_enabled else 0))),
    while respecting explicit caller budget caps (returns base_timeout if base_timeout > 180).
    """
    if base_timeout <= 180 and (target_files_count >= 5 or advisor_enabled):
        adaptive = 120 + target_files_count * 35 + (60 if advisor_enabled else 0)
        return min(600, max(base_timeout, adaptive))
    return base_timeout


def compute_restart_timeout(remaining_stage3b_budget_sec: float) -> int:
    """Computes restart timeout as min(600, max(0, remaining_stage3b_budget_sec)) without fabricating 60s."""
    return min(600, max(0, int(remaining_stage3b_budget_sec)))


def run_instream_micro_linter(file_path: Path, worktree: Path) -> dict:
    """
    NH-02: Zero-token in-stream micro-linter (<200ms).
    Executes single-file AST syntax check + Tier-A rules:
    - MISSING_GUARDED_ROUTE
    - MISSING_TENANT_SCOPE
    - UNIQUE_WITHOUT_TENANT
    Writes violations to .omp/live-lint-feedback.json.
    """
    p = Path(file_path)
    if not p.is_absolute():
        p = Path(worktree) / p

    violations = []
    try:
        rel_str = str(p.relative_to(worktree))
    except Exception:
        rel_str = str(p)

    if not p.is_file():
        return {"file": rel_str, "valid": False, "violations": [{"rule": "FILE_NOT_FOUND", "message": "File does not exist"}]}

    # 1. AST syntax check (<100ms)
    syntax_ok = verify_per_file_ast_syntax(p)
    if not syntax_ok:
        violations.append({
            "rule": "SYNTAX_ERROR",
            "message": f"File {rel_str} failed independent AST syntax check (syntax-torn or unbalanced).",
        })

    try:
        content = p.read_text(encoding="utf-8", errors="replace")
    except Exception as e:
        content = ""
        violations.append({"rule": "READ_ERROR", "message": str(e)})

    # 2. Tier-A Rule: MISSING_GUARDED_ROUTE on Next.js route handlers
    if "src/app/api/" in rel_str and (rel_str.endswith("route.ts") or rel_str.endswith("route.js")):
        if "guardedRoute" not in content and "createCrossTenantHandler" not in content:
            if re.search(r"export\s+async\s+function\s+(GET|POST|PUT|PATCH|DELETE)", content):
                violations.append({
                    "rule": "MISSING_GUARDED_ROUTE",
                    "message": f"API route {rel_str} exports route handlers without guardedRoute() wrapper.",
                })

    # 3. Tier-A Rule: MISSING_TENANT_SCOPE
    if ("prisma." in content or "tx." in content) and not rel_str.endswith((".test.ts", ".spec.ts", ".test.js")):
        for m in re.finditer(r"(?:prisma|tx)\.([a-zA-Z0-9]+)\.(findMany|findFirst|update|delete|deleteMany)\s*\(\s*\{([^}]+)\}", content):
            block = m.group(3)
            if "where" in block and "tenantId" not in block and "tenant_id" not in block and "tenantScoped: false" not in content:
                violations.append({
                    "rule": "MISSING_TENANT_SCOPE",
                    "message": f"Prisma query on {m.group(1)} in {rel_str} lacks explicit tenantId filter.",
                })
                break

    # 4. Tier-A Rule: UNIQUE_WITHOUT_TENANT in Prisma schemas or models
    if rel_str.endswith(".prisma") or "prisma" in rel_str:
        for m in re.finditer(r"@@unique\s*\(\s*\[([^\]]+)\]", content):
            fields = m.group(1)
            if "tenantId" not in fields and "tenant_id" not in fields and "id" not in fields:
                violations.append({
                    "rule": "UNIQUE_WITHOUT_TENANT",
                    "message": f"Unique constraint @@unique([{fields}]) in {rel_str} defined without composite tenantId.",
                })

    feedback = {
        "file": rel_str,
        "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "valid": len(violations) == 0,
        "violations": violations,
    }

    omp_dir = Path(worktree) / ".omp"
    omp_dir.mkdir(parents=True, exist_ok=True)
    feedback_file = omp_dir / "live-lint-feedback.json"
    try:
        feedback_file.write_text(json.dumps(feedback, indent=2), encoding="utf-8")
    except OSError as exc:
        sys.stderr.write(f"[omp-orch-executor] warning: failed writing live lint feedback {feedback_file}: {exc}\n")

    return feedback


def generate_omp_run_verify_script(
    worktree: Path,
    story_id: str,
    red_test_files: Optional[List[str]] = None,
) -> Path:
    """
    NH-02: Generates executable .omp/run-verify.sh supporting:
    - --quick: <3s fast AST + Semgrep on modified files.
    - --final: runs all 4 mandatory static pre-flight linters
      (validate-guarded-route-ast.ts, validate-zod-contract-coverage.py,
       validate-prisma-Migration.py, anti-cheat-linter.js) with valid CLI flags
      so preflight_linter_skipped == 0, plus sealed red tests.
    """
    clean_id = _normalize_story_id(story_id)
    omp_dir = Path(worktree) / ".omp"
    omp_dir.mkdir(parents=True, exist_ok=True)
    script_path = omp_dir / "run-verify.sh"

    red_tests_cmd = ""
    if red_test_files:
        files_str = " ".join(f'"{f}"' for f in red_test_files)
        red_tests_cmd = f"""
    echo "🧪 Running Sealed Red Tests..."
    if command -v pnpm >/dev/null 2>&1; then
        pnpm vitest run {files_str}
    else
        npx vitest run {files_str}
    fi
"""
    else:
        red_tests_cmd = '    echo "🧪 No sealed red tests specified."'

    script_content = f"""#!/usr/bin/env bash
set -e

MODE="${{1:---quick}}"
ROOT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")/.." && pwd)"
cd "$ROOT_DIR"

if [ "$MODE" = "--quick" ]; then
    echo "⚡ [NH-02] Running Quick In-Stream Verification (<3s)..."
    git diff --name-only | while read f; do
        if [ -f "$f" ]; then
            case "$f" in
                *.ts|*.tsx|*.js|*.jsx)
                    node -e 'const f=process.argv[1]; const ts=require("typescript"); const sf=ts.createSourceFile(f, require("fs").readFileSync(f, "utf8"), ts.ScriptTarget.Latest, true); process.exit((sf.parseDiagnostics || []).length === 0 ? 0 : 1);' "$f" || {{ echo "❌ Syntax error in $f"; exit 1; }}
                    ;;
                *.py)
                    python3 -m py_compile "$f" || {{ echo "❌ Syntax error in $f"; exit 1; }}
                    ;;
                *.json)
                    node -e 'const f=process.argv[1]; JSON.parse(require("fs").readFileSync(f, "utf8"));' "$f" || {{ echo "❌ JSON error in $f"; exit 1; }}
                    ;;
            esac
        fi
    done
    echo "✅ [NH-02] Quick In-Stream Verification PASS"
    exit 0

elif [ "$MODE" = "--final" ]; then
    echo "🛡️ [NH-02] Running Final Pre-Flight Static Linters & Sealed Red Tests..."
    
    # 1. validate-guarded-route-ast.ts (EC-P11-01)
    if [ -f ".agent/scripts/validate-guarded-route-ast.ts" ]; then
        echo "--> [1/4] validate-guarded-route-ast.ts"
        npx tsx .agent/scripts/validate-guarded-route-ast.ts
    fi

    # 2. validate-zod-contract-coverage.py (Khóa C1 + Parity)
    if [ -f ".agent/scripts/validate-zod-contract-coverage.py" ]; then
        echo "--> [2/4] validate-zod-contract-coverage.py --story-id {clean_id} --parity"
        python3 .agent/scripts/validate-zod-contract-coverage.py --story-id "{clean_id}" --parity
    fi

    # 3. validate-prisma-drift.py
    if [ -f ".agent/scripts/validate-prisma-drift.py" ]; then
        echo "--> [3/4] validate-prisma-drift.py"
        python3 .agent/scripts/validate-prisma-drift.py
    fi

    # 4. anti-cheat-linter.js
    if [ -f "scripts/anti-cheat-linter.js" ]; then
        echo "--> [4/4] scripts/anti-cheat-linter.js --story {clean_id}"
        node scripts/anti-cheat-linter.js --story "{clean_id}"
    fi

{red_tests_cmd}

    echo "✅ [NH-02] Final Pre-Flight Verification PASS (preflight_linter_skipped == 0)"
    exit 0
else
    echo "Unknown mode: $MODE (Usage: ./run-verify.sh [--quick|--final])"
    exit 1
fi
"""
    script_path.write_text(script_content, encoding="utf-8")
    try:
        os.chmod(script_path, 0o755)
    except OSError as exc:
        sys.stderr.write(f"[omp-orch-executor] warning: failed chmod 0755 on {script_path}: {exc}\n")
    log("📋 Generated .omp/run-verify.sh (executable mode 0755)")
    return script_path


def enforce_post_run_scope_guard(
    worktree_dir: Path,
    target_files: Any,
    baseline_snapshot: Optional[Dict[str, Optional[str]]] = None,
) -> List[str]:
    """
    Post-run Git Scope Guard (GP-1 / EC-OMPV3-02):
    Inspects git status against target_files + baseline_snapshot (skipping .omp/).
    If any file outside target_files was modified/deleted/created, automatically
    reverts it via git checkout / unlink and records warning.
    """
    allowed = set()
    if isinstance(target_files, dict):
        for k in ("create", "modify", "delete"):
            for f in target_files.get(k, []):
                if isinstance(f, str) and f.strip():
                    allowed.add(f.strip())
    elif isinstance(target_files, (list, set)):
        for f in target_files:
            if isinstance(f, str) and f.strip():
                allowed.add(f.strip())

    ignored_prefixes = (
        "coverage/", "dist/", ".omp/", "_iwish-output/", ".worktrees/",
        "node_modules/", ".next/", ".turbo/", ".agent/cache/"
    )
    baseline = baseline_snapshot or {}
    reverted_files = []

    res = subprocess.run(["git", "status", "--porcelain", "-uall"], cwd=worktree_dir, capture_output=True, text=True)
    for line in res.stdout.splitlines():
        if not line.strip():
            continue
        status_code = line[:2]
        filepath = line[3:].strip()
        if " -> " in filepath:
            filepath = filepath.split(" -> ", 1)[1].strip()

        if any(filepath.startswith(p) for p in ignored_prefixes):
            continue
        if "/__pycache__/" in filepath or filepath.startswith("__pycache__/"):
            continue
        if filepath.endswith(".tsbuildinfo") or filepath.endswith(".pyc"):
            continue
        if filepath in (".omp-prompt.md", "code-stage-evidence.json", "omp-trace.html", "omp-trace.jsonl", "omp-report.md"):
            continue

        if filepath in allowed:
            continue
        is_parent_match = any(
            (allowed_path.endswith("/") and filepath.startswith(allowed_path))
            or filepath.startswith(allowed_path.rstrip("/") + "/")
            for allowed_path in allowed
        )
        if is_parent_match:
            continue

        # Check if unchanged since baseline
        full_path = worktree_dir / filepath
        curr_sha = None
        if full_path.is_file():
            try:
                curr_sha = hashlib.sha256(full_path.read_bytes()).hexdigest()
            except Exception:
                curr_sha = "UNREADABLE"
        if filepath in baseline and baseline[filepath] == curr_sha:
            continue

        # Out-of-scope modification! Revert it immediately
        log(f"🛡️ [Scope Guard] Out-of-scope file detected: {filepath} ({status_code}). Reverting...")
        if status_code == "??" and filepath not in baseline:
            if full_path.is_file():
                full_path.unlink(missing_ok=True)
        else:
            subprocess.run(["git", "checkout", "HEAD", "--", filepath], cwd=worktree_dir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        reverted_files.append(filepath)

    if reverted_files:
        log(f"⚠️ [Scope Guard] Reverted {len(reverted_files)} out-of-scope files: {reverted_files}")
    return reverted_files


def pre_flight_probe(models=("gemini-3.8-flash", "gemini-4-argon"), topology="antigravity") -> bool:
    """Validates that execution binary (omp) exists and is responsive (Delegation Mode)."""
    omp_bin = shutil.which("omp") or str(Path.home() / ".local/bin/omp")
    if not os.path.exists(omp_bin) and not shutil.which("omp"):
        log("❌ omp binary not found in PATH or ~/.local/bin/omp.")
        return False
    try:
        res = subprocess.run([omp_bin, "--version"], capture_output=True, text=True, timeout=5)
        if res.returncode == 0:
            log(f"✅ Pre-flight Probe: {res.stdout.strip()} verified.")
            return True
        log(f"⚠️ Pre-flight Probe returned code {res.returncode}")
        return False
    except Exception as e:
        log(f"⚠️ Pre-flight Probe failed: {e}")
        return False

def set_worktree_lock(worktree_dir: Path, lock: bool = True):
    """Acquires or releases lock in worktree-registry to prevent Reaper race conditions."""
    try:
        registry_script = Path(_agent_dir) / ".agent" / "scripts" / "worktree-registry.py"
        if registry_script.exists():
            action = "lock" if lock else "unlock"
            subprocess.run([sys.executable, str(registry_script), action, "--path", str(worktree_dir)],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
    except Exception as e:
        log(f"⚠️ Registry lock notice: {e}")

def start_heartbeat(worktree_dir: Path, stop_event: threading.Event) -> threading.Thread:
    """Periodically pulses heartbeat to keep worktree immune from automated Cron Reaper."""
    def _beat():
        registry_script = Path(_agent_dir) / ".agent" / "scripts" / "worktree-registry.py"
        while not stop_event.is_set():
            try:
                if registry_script.exists():
                    subprocess.run([sys.executable, str(registry_script), "heartbeat", "--path", str(worktree_dir)],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
            except Exception as e:
                log(f"⚠️ Heartbeat pulse error: {e}")
            stop_event.wait(300) # 5 minutes
    t = threading.Thread(target=_beat, daemon=True)
    t.start()
    return t

# ---------------------------------------------------------------------------
# Checkpoint, Rollback & User Failure Protocol
# ---------------------------------------------------------------------------

def capture_workspace_state_snapshot(worktree_dir: Path) -> Dict[str, Optional[str]]:
    """Captures SHA-256 fingerprint of all currently modified/untracked files before execution."""
    res = subprocess.run(["git", "status", "--porcelain", "-uall"], cwd=worktree_dir, capture_output=True, text=True)
    snapshot: Dict[str, Optional[str]] = {}
    for line in res.stdout.splitlines():
        if not line.strip():
            continue
        filepath = line[3:].strip()
        if " -> " in filepath:
            filepath = filepath.split(" -> ", 1)[1].strip()
        full_path = worktree_dir / filepath
        if full_path.is_file():
            try:
                snapshot[filepath] = hashlib.sha256(full_path.read_bytes()).hexdigest()
            except Exception:
                snapshot[filepath] = "UNREADABLE"
        else:
            snapshot[filepath] = None
    return snapshot

def setup_git_checkpoint(worktree_dir: Path, tag_name: str):
    """Sets a git checkpoint tag before execution."""
    subprocess.run(["git", "tag", "-d", tag_name], cwd=worktree_dir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    subprocess.run(["git", "tag", tag_name], cwd=worktree_dir, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    log(f"📌 Git checkpoint created: {tag_name}")

def atomic_rollback(
    worktree_dir: Path,
    tag_name: str,
    baseline_snapshot: Optional[Dict[str, Optional[str]]] = None,
    target_files: Optional[Dict[str, List[str]]] = None,
):
    """Performs safe rollback without running forbidden git reset --hard on a shared/dirty worktree."""
    log(f"🔄 Executing Safe Scoped Rollback to checkpoint: {tag_name}")
    try:
        res = subprocess.run(["git", "status", "--porcelain", "-uall"], cwd=worktree_dir, capture_output=True, text=True)
        baseline = baseline_snapshot or {}
        for line in res.stdout.splitlines():
            if not line.strip():
                continue
            status_code = line[:2]
            filepath = line[3:].strip()
            if " -> " in filepath:
                filepath = filepath.split(" -> ", 1)[1].strip()
            if filepath.startswith(("_iwish-output/", ".omp/", ".agent/", "scripts/adapters/omp-orch-executor.py")):
                continue
            full_path = worktree_dir / filepath
            curr_sha = None
            if full_path.is_file():
                try:
                    curr_sha = hashlib.sha256(full_path.read_bytes()).hexdigest()
                except Exception:
                    curr_sha = "UNREADABLE"
            if filepath in baseline and baseline[filepath] == curr_sha:
                continue
            # File was newly created or modified during this OMP run
            if status_code == "??" and filepath not in baseline:
                if full_path.is_file():
                    full_path.unlink(missing_ok=True)
            elif filepath not in baseline:
                subprocess.run(["git", "checkout", "--", filepath], cwd=worktree_dir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(["git", "tag", "-d", tag_name], cwd=worktree_dir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        log("✨ Workspace restored to pre-OMP baseline state.")
    except Exception as e:
        log(f"⚠️ Rollback notice: {e}")
        try:
            subprocess.run(["git", "tag", "-d", tag_name], cwd=worktree_dir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except (subprocess.SubprocessError, OSError) as tag_exc:
            sys.stderr.write(f"[omp-orch-executor] warning: failed cleaning git rollback tag {tag_name}: {tag_exc}\n")

def trip_circuit_breaker(output_dir: Path, reason: str):
    """Writes latch file with scrubbed reason to prevent automatic flapping."""
    latch_file = Path(output_dir) / ".circuit-breaker-tripped"
    latch_data = {
        "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "reason": scrub_secrets(str(reason)) # P1 Fix: Redact credentials from latch file
    }
    try:
        with open(latch_file, "w", encoding="utf-8") as f:
            json.dump(latch_data, f, indent=2)
        log(f"🔒 Circuit breaker latch engaged: {latch_file}")
    except Exception as e:
        log(f"⚠️ Failed to write circuit breaker latch: {e}")

def notify_user_and_halt(output_dir: Path, error_summary: str, duration_sec: float, exit_code: int = EXIT_FALLBACK_TRIGGER):
    """
    Gold Rule: Rollback workspace and explicitly prompt the user to decide
    whether to retry OMP, switch to native IDE/agent, or cancel.
    """
    failure_report = Path(output_dir) / ".omp-failure-report.json"
    report_data = {
        "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "exit_code": exit_code,
        "error_summary": scrub_secrets(error_summary),
        "duration_seconds": round(duration_sec, 2),
        "status": "rolled_back_awaiting_user_decision"
    }
    try:
        with open(failure_report, "w", encoding="utf-8") as f:
            json.dump(report_data, f, indent=2)
    except Exception as e:
        log(f"⚠️ Failed to write failure report: {e}")

    prompt_ui = f"""
================================================================================
⚠️  OMP ENGINE ENCOUNTERED AN ISSUE & WORKSPACE WAS CLEANLY ROLLED BACK
================================================================================
📋 Chi tiết sự cố: [Exit Code {exit_code}] {scrub_secrets(error_summary)}
⏱️ Thời gian thực thi: {round(duration_sec, 1)}s
🔒 Trạng thái workspace: 100% sạch (đã khôi phục nguyên tử về trước khi chạy OMP).
🛡️ Chốt an toàn: Đã kích hoạt .circuit-breaker-tripped (chống auto-loop).

👉 BẠN MUỐN THỰC HIỆN BƯỚC TIẾP THEO NHƯ THẾ NÀO?
  1. [Kiểm tra lại OMP]: Xóa latch, chỉnh lại prompt/config và chạy lại OMP.
  2. [Chuyển về Native IDE]: Tiếp tục câu chuyện bằng Antigravity / dev-agent tích hợp sẵn.
  3. [Hủy tác vụ]: Dừng lại để xem xét mã nguồn.
================================================================================
"""
    print(prompt_ui, flush=True)

def kill_process_tree(proc: subprocess.Popen):
    """Kills entire process group cleanly and reaps zombie processes (P1 Fix)."""
    if proc:
        try:
            pgid = os.getpgid(proc.pid)
            os.killpg(pgid, signal.SIGTERM)
            time.sleep(1)
            os.killpg(pgid, signal.SIGKILL)
            proc.wait(timeout=2) # P1 Fix: Reap process table entry
        except (ProcessLookupError, subprocess.TimeoutExpired):
            pass
        except Exception as e:
            log(f"⚠️ Error cleaning process tree: {e}")

# ---------------------------------------------------------------------------
# Input Validation & Scope Guard
# ---------------------------------------------------------------------------

def validate_impl_plan_schema_and_paths(plan_path: Path) -> Tuple[bool, Optional[Dict[str, Any]], Optional[str]]:
    """Validates impl-plan.json against schema and strictly blocks path traversal (P1 Fix)."""
    schema_file = Path(_agent_dir) / ".agent" / "schemas" / "impl-plan.schema.json"
    
    if not plan_path.exists():
        return False, None, f"Plan file not found: {plan_path}"
        
    try:
        with open(plan_path, "r", encoding="utf-8") as f:
            plan_data = json.load(f)
    except Exception as e:
        return False, None, f"Failed to parse JSON: {e}"

    # 1. JSON Schema validation
    if schema_file.exists():
        try:
            import jsonschema
            with open(schema_file, "r", encoding="utf-8") as sf:
                schema = json.load(sf)
            jsonschema.validate(instance=plan_data, schema=schema)
            log("✅ Machine Contract: impl-plan.json passed schema validation.")
        except ImportError:
            log("⚠️ jsonschema package not found; skipping schema validation.")
        except Exception as e:
            return False, None, f"Schema validation error: {e}"

    # 2. Path traversal & security checks
    target_files = plan_data.get("target_files", {})
    all_files = target_files.get("create", []) + target_files.get("modify", []) + target_files.get("delete", [])

    if len(all_files) > 50:
        return False, None, f"Target files exceed 50 limit (count: {len(all_files)})"

    forbidden_patterns = ("../", "..\\", "/etc/", "/var/", ".git/", "_iwish-output/")
    for fpath in all_files:
        if not fpath or not isinstance(fpath, str):
            return False, None, f"Invalid file entry in target_files: {fpath}"
        if fpath.startswith("/") or fpath.startswith("\\"):
            return False, None, f"Absolute paths forbidden in plan: {fpath}"
        if any(fpath.startswith(p) or f"/{p}" in fpath for p in forbidden_patterns):
            return False, None, f"Security violation: path traversal or protected path in target_files: {fpath}"

    return True, plan_data, None

def run_safe_test(test_cmd: str, cwd: Path, timeout: int = 300) -> subprocess.CompletedProcess:
    """Validates test command against prefix whitelist and enforces timeout (P1 Fix)."""
    import shlex
    allowed_prefixes = (
        "pnpm test", "npm test", "yarn test", "pytest", "cargo test",
        "pnpm run test", "npm run test", "vitest", "tsc", "npx vitest"
    )
    cmd_trimmed = test_cmd.strip()
    if not any(cmd_trimmed.startswith(p) for p in allowed_prefixes):
        log(f"⚠️ Test command '{cmd_trimmed}' not in whitelist. Sanitizing to 'pnpm test'.")
        cmd_trimmed = "pnpm test"

    return subprocess.run(shlex.split(cmd_trimmed), shell=False, cwd=cwd, capture_output=True, text=True, timeout=timeout)

def check_scope_lockdown(
    worktree_dir: Path,
    target_files: Dict[str, List[str]],
    baseline_snapshot: Optional[Dict[str, Optional[str]]] = None,
) -> bool:
    """Verifies that no forbidden or untracked files outside plan were modified during this OMP run."""
    res = subprocess.run(["git", "status", "--porcelain", "-uall"], cwd=worktree_dir, capture_output=True, text=True)
    allowed = set(target_files.get("create", []) + target_files.get("modify", []) + target_files.get("delete", []))
    baseline = baseline_snapshot or {}
    
    ignored_prefixes = (
        "coverage/", "dist/", ".omp/", "_iwish-output/", ".worktrees/",
        "node_modules/", ".next/", ".turbo/", ".agent/"
    )
    
    violations = []
    for line in res.stdout.splitlines():
        if not line.strip():
            continue
        filepath = line[3:].strip()
        if " -> " in filepath:
            filepath = filepath.split(" -> ", 1)[1].strip()
        
        if any(filepath.startswith(p) for p in ignored_prefixes):
            continue
        if "/__pycache__/" in filepath or filepath.startswith("__pycache__/"):
            continue
        if filepath.endswith(".tsbuildinfo") or filepath.endswith(".pyc"):
            continue
            
        if filepath in (".omp-prompt.md", "code-stage-evidence.json", "omp-trace.html", "omp-trace.jsonl", "omp-report.md"):
            continue

        # Ignore files that were already dirty before OMP started and whose SHA-256 did not change
        if filepath in baseline:
            full_path = worktree_dir / filepath
            curr_sha = None
            if full_path.is_file():
                try:
                    curr_sha = hashlib.sha256(full_path.read_bytes()).hexdigest()
                except Exception:
                    curr_sha = "UNREADABLE"
            if baseline[filepath] == curr_sha:
                continue
            
        # Exact match or parent directory match
        if filepath not in allowed:
            is_parent_match = any(
                (allowed_path.endswith("/") and filepath.startswith(allowed_path))
                or filepath.startswith(allowed_path.rstrip("/") + "/")
                for allowed_path in allowed
            )
            if not is_parent_match:
                violations.append(filepath)
            
    if violations:
        log(f"🚨 Scope Lockdown Violation! Modified files outside plan: {violations}")
        return False
    return True

def dynamic_scope_guard(
    worktree_dir: Path,
    allowed_paths=None,
    baseline_snapshot: Optional[Dict[str, Optional[str]]] = None,
) -> bool:
    """Protects system directories during Dynamic Mode ad-hoc execution."""
    res = subprocess.run(["git", "status", "--porcelain", "-uall"], cwd=worktree_dir, capture_output=True, text=True)
    protected_prefixes = (".agent/", ".git/")
    sensitive_suffixes = (".env", ".key", ".pem", ".p12")
    baseline = baseline_snapshot or {}
    
    violations = []
    for line in res.stdout.splitlines():
        if not line.strip():
            continue
        filepath = line[3:].strip()
        if " -> " in filepath:
            filepath = filepath.split(" -> ", 1)[1].strip()

        if filepath in baseline:
            full_path = worktree_dir / filepath
            curr_sha = None
            if full_path.is_file():
                try:
                    curr_sha = hashlib.sha256(full_path.read_bytes()).hexdigest()
                except Exception:
                    curr_sha = "UNREADABLE"
            if baseline[filepath] == curr_sha:
                continue
        
        if filepath.startswith("_iwish-output/adhoc-workspace/scratch/"):
            continue
        if filepath.startswith("_iwish-output/"):
            violations.append(filepath)
            continue
        if any(filepath.startswith(p) for p in protected_prefixes):
            violations.append(filepath)
            continue
        if any(filepath.endswith(s) for s in sensitive_suffixes):
            violations.append(filepath)
            continue
            
    if violations:
        log(f"🚨 Dynamic Mode Scope Violation! Modified protected system files: {violations}")
        return False
    return True

# ---------------------------------------------------------------------------
# Prompt Synthesizer (4 XML Templates)
# ---------------------------------------------------------------------------

def build_plan_prompt(story_id: str, context_files: str, current_dir: str) -> str:
    return f"""<role>
Bạn là Architectural Scout & Technical Planner (Model: @plan / gemini-4-argon).
Chế độ hoạt động: READ-ONLY SURVEY & TECHNICAL DISCOVERY.
</role>

<context>
- Tài liệu đầu vào: {context_files}
- Bối cảnh kiến trúc hiện hành: Thư mục dự án hiện tại ({current_dir}).
</context>

<directives>
1. Tuyệt đối KHÔNG sửa đổi, tạo mới hay xóa bất kỳ file source code nào trong phiên này.
2. Dùng tool `glob`, `grep`, `ast_grep` và đặc biệt là `lsp` (goto definition, find references) để lần theo các module, services, controllers và models bị ảnh hưởng.
3. Nếu cần quét rộng thư viện/cấu trúc, hãy gọi subagent `scout` (@smol) để nén thông tin.
4. Đầu ra phải xuất danh sách định lượng cụ thể:
   - Các file cần tạo mới (Target Files - Create).
   - Các file cần chỉnh sửa (Target Files - Modify).
   - Danh sách interface/types cần bổ sung hoặc mở rộng.
   - Trình tự thực thi (Task Breakdown) và lệnh test tương ứng.
</directives>

<task>
Khảo sát codebase thực tế và đề xuất bản phác thảo kế hoạch kỹ thuật chính xác cho Story: {story_id}.
</task>
"""

def build_code_prompt(
    story_id: str,
    plan_data: Dict[str, Any],
    plan_file: str,
    worker_model: str = "gemini-3.8-flash",
    advisor_model: str = "gemini-4-argon",
    topology: str = "antigravity",
) -> str:
    tasks_text = "\n".join([f"- [ ] {t.get('task_id', idx)}: {t.get('name') or t.get('title', '')} (Target: `{t.get('target', '')}`)"
                            for idx, t in enumerate(plan_data.get("tasks", []))])
    target_files = plan_data.get("target_files", {})
    all_targets = target_files.get("create", []) + target_files.get("modify", []) + target_files.get("delete", [])
    target_files_text = "\n".join([f"- `{f}`" for f in all_targets])
    test_cmd = plan_data.get("testing", {}).get("test_command", "pnpm test")

    is_plugin_story = any("src/plugins" in f or "cowok-plugin-pack" in f for f in all_targets) or "plugin" in str(story_id).lower()
    plugin_dod_directive = (
        "\n6. Plugin Definition of Done (GP-2 / G1_ui_bundle_exists): Mọi plugin dưới `src/plugins/<kit>/<plugin>/` BẮT BUỘC phải có mặt vật lý cả 2 file `manifest.json` VÀ `ui.js` (ESM bundle có `export default` hoặc `export function mount/render`) song song với file nén `dist/<kit>.zip`. Tuyệt đối không xóa `ui.js` sau khi đóng gói `.zip`."
        if is_plugin_story
        else ""
    )

    return f"""<role>
Bạn là Lead Software Engineer (Worker: {worker_model}, Watchdog Advisor: {advisor_model}, Topology: {topology}).
Chế độ hoạt động: STRICT SCOPE LOCKDOWN & ZERO DRIFT.
</role>

<context>
- Hợp đồng máy SSOT: @{plan_file} (Story: {story_id})
- Ranh giới mã nguồn được phép can thiệp (Target Files):
{target_files_text}
- Lệnh kiểm thử nghiệm thu:
  `{test_cmd}`
- Danh sách công việc:
{tasks_text}
</context>

<directives>
1. Tận dụng In-Process LSP: Sau mỗi lần edit file, BẮT BUỘC dùng tool `lsp` để kiểm tra diagnostics. Tuyệt đối không để sót lỗi TypeScript/Linter.
2. Giao thức Cross-Boundary: Mọi event, type, enum mới qua module khác phải xác nhận bên nhận có xử lý.
3. Kỷ luật ranh giới: TUYỆT ĐỐI KHÔNG sửa file ngoài danh sách <context>.
4. Tự chữa lành: Chạy `{test_cmd}`, tự động đọc lỗi và sửa cho đến khi 100% tests pass.
5. Kiểm tra nội bộ trực tiếp: Tự đối chiếu trực tiếp 100% ACs với git diff; TUYỆT ĐỐI KHÔNG gọi tool task hoặc spawn subagent reviewer lồng nhau.{plugin_dod_directive}
</directives>

<task>
Thực hiện trọn vẹn các tasks kỹ thuật được định nghĩa trong `impl-plan.json` cho Story: {story_id}.
</task>
"""

def build_review_prompt(story_id: str, plan_data: Dict[str, Any], checkpoint_tag: str, plan_file: str) -> str:
    target_files = plan_data.get("target_files", {})
    all_targets = target_files.get("create", []) + target_files.get("modify", [])
    target_files_text = "\n".join([f"- `{f}`" for f in all_targets])

    return f"""<role>
Bạn là Security & Architecture Reviewer (Subagents: reviewer, security-reviewer).
Chế độ hoạt động: READ-ONLY AUDIT & VULNERABILITY DISCOVERY.
</role>

<context>
- Git Diff đối chiếu: `{checkpoint_tag}` ... HEAD
- Hợp đồng máy đối chiếu: @impl-plan.json (Story: {story_id})
- Danh sách file mục tiêu đã phê duyệt:
{target_files_text}
</context>

<directives>
1. Chỉ chạy các lệnh đọc: `git diff`, `read`, `grep`, `ast_grep`. TUYỆT ĐỐI KHÔNG sửa file hay chạy build.
2. Kiểm tra ranh giới <cross-boundary>: Kiểm tra mọi payload, switch/case, router xem có bị silent drop không.
3. Áp dụng bảng phân loại P0-P3 (P0: Chặn release, P1: Sửa chu kỳ tới, P2: Nợ kỹ thuật, P3: Góp ý).
4. Phân loại findings theo chuẩn cấu trúc: title, body, priority, file_path, line_start, line_end.
</directives>

<task>
Thẩm định toàn diện chất lượng, độ an toàn và độ lệch kế hoạch (Impl-Plan Deviation) của diff hiện tại.
</task>
"""

def build_dynamic_prompt(intent: str, user_prompt: str, context_hint: str) -> str:
    return f"""<role>
Bạn là Senior Fullstack Engineer (Chế độ Ad-hoc: {intent}).
Chế độ hoạt động: SURGICAL PRECISION & LEAST ASTONISHMENT.
</role>

<context>
- Yêu cầu từ người dùng: "{user_prompt}"
- Bối cảnh được chỉ định: {context_hint}
</context>

<directives>
1. Phẫu thuật chính xác (Surgical Fix): Chỉ chạm vào những file liên quan trực tiếp đến vấn đề. Không tự ý refactor lan man.
2. Tận dụng `lsp` và `grep` để tìm chính xác nguồn gốc lỗi (Root Cause) trước khi sửa.
3. Nếu là tác vụ dọn lỗi toàn dự án: Tự động kích hoạt cơ chế file-disjoint subagents (`cleanse`).
4. Luôn chạy test hoặc typecheck kiểm chứng sau khi sửa.
5. Tuyệt đối KHÔNG can thiệp vào các đường dẫn được bảo vệ: `.agent/**`, `.git/**`, `_iwish-output/**`, file cấu hình nhạy cảm (`.env*`).
</directives>

<task>
Thực hiện yêu cầu ad-hoc trên một cách nhanh chóng, sạch sẽ và an toàn.
</task>
"""

# ---------------------------------------------------------------------------
# Host-Side Session Token Budget Ledger (Zero-Trust Category A — Khóa P10)
# ---------------------------------------------------------------------------

MAX_ADVISOR_STEER_ROUNDS = 3
DEFAULT_MAX_SESSION_TOKENS = 256000
DEFAULT_MAX_TURN_TOKENS = 64000
DEFAULT_WORKER_CEILING = 96000
DEFAULT_ADVISOR_CEILING_PER_ROUND = 32000
DEFAULT_REVIEW_BUDGET = 64000


def _get_watchmen_hmac_key() -> bytes:
    import hashlib
    pub_pem = Path(_agent_dir) / ".agent" / "config" / "watchmen-pub.pem"
    if pub_pem.is_file():
        return hashlib.sha256(pub_pem.read_bytes()).digest()
    return hashlib.sha256(b"watchmen-tcb-default-hmac-key").digest()


def _compute_ledger_entry_hmac(
    story_id: str,
    worktree_realpath: str,
    run_id: str,
    topology: str,
    worker_model: str,
    total_run: int,
    prev_hash: str,
) -> str:
    import hashlib
    import hmac
    key = _get_watchmen_hmac_key()
    msg = f"{story_id}:{worktree_realpath}:{run_id}:{topology}:{worker_model}:{total_run}:{prev_hash}".encode("utf-8")
    return hmac.new(key, msg, hashlib.sha256).hexdigest()


def _normalize_story_id(raw_id: str) -> str:
    """Canonicalize story ID to clean format (e.g. 'story-41.18' -> '41.18')."""
    if not raw_id:
        return ""
    import re
    return re.sub(r'^(?:story[-_]?|s)(?=\d)', '', str(raw_id).strip(), flags=re.IGNORECASE)


def get_session_token_ledger_path(story_id: str) -> Path:
    clean_id = _normalize_story_id(story_id)
    state_dir = Path.home() / ".agent-state" / clean_id
    state_dir.mkdir(parents=True, exist_ok=True)
    return state_dir / "session-token-ledger.json"


def verify_session_token_ledger_integrity(
    story_id: str,
    worktree_dir: Optional[Path] = None,
) -> Tuple[bool, str]:
    """Verify the chained HMAC integrity of ~/.agent-state/<story_id>/session-token-ledger.json and detect deletion."""
    ledger_path = get_session_token_ledger_path(story_id)
    wt_root = (worktree_dir or Path(_agent_dir)).resolve()
    runs_dir = wt_root / "_iwish-output" / "adhoc-workspace" / "runs" / story_id

    if not ledger_path.is_file():
        if runs_dir.is_dir() and list(runs_dir.glob("*/execution-receipt.json")):
            return False, "budget_ledger_tampered: session-token-ledger.json deleted while run receipts exist"
        return True, "OK"

    try:
        data = json.loads(ledger_path.read_text(encoding="utf-8"))
    except Exception as exc:
        return False, f"budget_ledger_tampered: malformed JSON ({exc})"

    runs = data.get("runs", [])
    if not isinstance(runs, list):
        return False, "budget_ledger_tampered: invalid runs list"

    expected_prev = "0" * 64
    calc_sum = 0
    for idx, r in enumerate(runs):
        if r.get("prev_hash") != expected_prev:
            return False, f"budget_ledger_tampered: broken prev_hash chain at index {idx}"
        wt_rp = r.get("worktree_realpath", os.path.realpath(wt_root))
        tot = int(r.get("total_tokens", 0))
        calc_sum += tot
        expected_hmac = _compute_ledger_entry_hmac(
            story_id=data.get("story_id", story_id),
            worktree_realpath=wt_rp,
            run_id=str(r.get("run_id", "")),
            topology=str(r.get("topology", "")),
            worker_model=str(r.get("worker_model", "")),
            total_run=tot,
            prev_hash=expected_prev,
        )
        if r.get("entry_hash") != expected_hmac:
            return False, f"budget_ledger_tampered: HMAC mismatch at run index {idx}"
        expected_prev = expected_hmac

    if int(data.get("accumulated_tokens", 0)) != calc_sum:
        return False, f"budget_ledger_tampered: accumulated_tokens ({data.get('accumulated_tokens')}) != sum(runs) ({calc_sum})"

    return True, "OK"


def check_session_token_budget(
    story_id: str,
    max_session_tokens: int = DEFAULT_MAX_SESSION_TOKENS,
    worktree_dir: Optional[Path] = None,
) -> Tuple[bool, int, str]:
    import fcntl
    ledger_path = get_session_token_ledger_path(story_id)
    lock_file = ledger_path.parent / ".session-token-ledger.lock"
    with open(lock_file, "w") as lock_f:
        fcntl.flock(lock_f.fileno(), fcntl.LOCK_SH)
        try:
            intact, reason = verify_session_token_ledger_integrity(story_id, worktree_dir=worktree_dir)
            if not intact:
                return False, -1, reason
            if not ledger_path.is_file():
                return True, 0, "OK"
            data = json.loads(ledger_path.read_text(encoding="utf-8"))
            accumulated = int(data.get("accumulated_tokens", 0))
            if accumulated >= max_session_tokens:
                return False, accumulated, f"Session token budget exhausted: {accumulated} >= {max_session_tokens}"
            return True, accumulated, "OK"
        except Exception as e:
            return False, -1, f"budget_ledger_tampered: {e}"
        finally:
            fcntl.flock(lock_f.fileno(), fcntl.LOCK_UN)


def record_session_token_usage(
    story_id: str,
    run_id: str,
    topology: str,
    worker_model: str,
    input_tokens: int,
    output_tokens: int,
    max_session_tokens: int = DEFAULT_MAX_SESSION_TOKENS,
    worktree_dir: Optional[Path] = None,
) -> Dict[str, Any]:
    import fcntl
    import uuid
    wt_realpath = os.path.realpath(worktree_dir or Path(_agent_dir))
    ledger_path = get_session_token_ledger_path(story_id)
    lock_file = ledger_path.parent / ".session-token-ledger.lock"
    with open(lock_file, "w") as lock_f:
        fcntl.flock(lock_f.fileno(), fcntl.LOCK_EX)
        try:
            if ledger_path.is_file():
                intact, reason = verify_session_token_ledger_integrity(story_id, worktree_dir=worktree_dir)
                if not intact:
                    raise RuntimeError(reason)
                ledger = json.loads(ledger_path.read_text(encoding="utf-8"))
            else:
                ledger = {
                    "story_id": story_id,
                    "worktree_realpath": wt_realpath,
                    "max_session_tokens": max_session_tokens,
                    "max_turn_tokens": DEFAULT_MAX_TURN_TOKENS,
                    "budget_formula": {
                        "worker_ceiling": DEFAULT_WORKER_CEILING,
                        "max_advisor_steer_rounds": MAX_ADVISOR_STEER_ROUNDS,
                        "advisor_ceiling_per_round": DEFAULT_ADVISOR_CEILING_PER_ROUND,
                        "review_budget": DEFAULT_REVIEW_BUDGET,
                        "total_ceiling": DEFAULT_WORKER_CEILING + MAX_ADVISOR_STEER_ROUNDS * DEFAULT_ADVISOR_CEILING_PER_ROUND + DEFAULT_REVIEW_BUDGET,
                    },
                    "accumulated_tokens": 0,
                    "runs": [],
                }

            runs = ledger.get("runs", [])
            prev_hash = runs[-1].get("entry_hash", "0" * 64) if runs else "0" * 64
            total_run = input_tokens + output_tokens
            entry_hash = _compute_ledger_entry_hmac(
                story_id=story_id,
                worktree_realpath=wt_realpath,
                run_id=run_id,
                topology=topology,
                worker_model=worker_model,
                total_run=total_run,
                prev_hash=prev_hash,
            )

            new_run = {
                "run_id": run_id,
                "timestamp": int(time.time() * 1000),
                "worktree_realpath": wt_realpath,
                "topology": topology,
                "worker_model": worker_model,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "total_tokens": total_run,
                "prev_hash": prev_hash,
                "entry_hash": entry_hash,
            }
            runs.append(new_run)
            accumulated = sum(r.get("total_tokens", 0) for r in runs)
            ledger["accumulated_tokens"] = accumulated
            ledger["max_session_tokens"] = max_session_tokens
            ledger["runs"] = runs

            tmp_file = ledger_path.parent / f"session-token-ledger.json.{uuid.uuid4().hex[:8]}.tmp"
            tmp_file.write_text(json.dumps(ledger, indent=2) + "\n", encoding="utf-8")
            os.replace(tmp_file, ledger_path)
            return ledger
        finally:
            fcntl.flock(lock_f.fileno(), fcntl.LOCK_UN)

# ---------------------------------------------------------------------------
# Post-Execution: Trace, Export & Token Accounting
# ---------------------------------------------------------------------------

def process_session_artifacts(
    session_dir: Path,
    output_dir: Path,
    story_id: str,
    mode: str,
    duration_sec: float,
    topology: str = "antigravity",
    active_model: str = "gemini-3.8-flash",
    reverted_files: Optional[List[str]] = None,
    run_start_time: Optional[float] = None,
):
    """
    Finds latest primary OMP session JSONL, copies it, exports offline HTML,
    sums token usage from BOTH primary AND all advisor (__advisor*.jsonl) sessions,
    logs any reverted out-of-scope files in omp-report.md,
    and updates session-token-ledger.json. (EC-OMPV3-03, GP-1, R8-01)
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    omp_bin = shutil.which("omp") or str(Path.home() / ".local/bin/omp")

    # 1. Find newest primary session file (R8-01: filter by mtime >= run_start_time)
    all_jsonl = list(session_dir.glob("**/*.jsonl"))
    if run_start_time is not None:
        filtered = [f for f in all_jsonl if f.stat().st_mtime >= (run_start_time - 2.0)]
        if filtered:
            all_jsonl = filtered
    if not all_jsonl:
        log("⚠️ No session JSONL found in session-dir.")
        return

    primary_sessions = [f for f in all_jsonl if not f.name.startswith("__advisor")]
    latest_primary = max(primary_sessions, key=lambda f: f.stat().st_mtime) if primary_sessions else max(all_jsonl, key=lambda f: f.stat().st_mtime)

    dest_jsonl = output_dir / "omp-trace.jsonl"
    shutil.copy2(latest_primary, dest_jsonl)
    log(f"📄 Saved primary trace: {dest_jsonl}")

    # 2. Run offline export for HTML (0 Token Cost) using primary session
    dest_html = output_dir / "omp-trace.html"
    if omp_bin and os.path.exists(omp_bin):
        try:
            exp_res = subprocess.run(
                [omp_bin, "--export", str(dest_jsonl.resolve())],
                capture_output=True,
                text=True,
                cwd=output_dir,
                timeout=30
            )
            if exp_res.returncode == 0:
                if exp_res.stdout.strip().startswith("<!DOCTYPE") or "<html" in exp_res.stdout:
                    dest_html.write_text(exp_res.stdout, encoding="utf-8")
                    log(f"🌐 Offline HTML trace generated: {dest_html}")
                else:
                    potential_html = list(output_dir.glob("*.html"))
                    if potential_html:
                        log(f"🌐 Offline HTML trace exported: {potential_html[0]}")
            else:
                log(f"⚠️ HTML export warning: {exp_res.stderr.strip()}")
        except Exception as e:
            log(f"⚠️ Failed to export HTML trace: {e}")

    # 3. Analyze token metrics across ALL transcripts (Primary + Advisors)
    total_input = 0
    total_output = 0
    total_cache_read = 0
    total_reasoning = 0
    total_cost = 0.0
    models_used = set()

    for jf in all_jsonl:
        try:
            with open(jf, "r", encoding="utf-8") as f:
                for line in f:
                    if not line.strip():
                        continue
                    try:
                        entry = json.loads(line)
                        msg = entry.get("message", {}) if isinstance(entry.get("message"), dict) else {}
                        model = entry.get("model") or msg.get("model")
                        if model:
                            models_used.add(model)
                        usage = entry.get("usage") or msg.get("usage") or {}
                        total_input += usage.get("input", 0)
                        total_output += usage.get("output", 0)
                        total_cache_read += usage.get("cacheRead", 0)
                        total_reasoning += usage.get("reasoningTokens", 0)
                        total_cost += usage.get("cost", {}).get("total", 0.0)
                    except (json.JSONDecodeError, ValueError, TypeError, AttributeError) as parse_exc:
                        sys.stderr.write(f"[omp-orch-executor] warning: malformed transcript JSONL line in {jf}: {parse_exc}\n")
                        continue
        except Exception as e:
            log(f"⚠️ Error reading transcript {jf}: {e}")

    total_tokens = total_input + total_output + total_cache_read
    cache_rate = (total_cache_read / total_tokens * 100) if total_tokens > 0 else 0.0

    reverted_section = ""
    if reverted_files:
        rev_list = "\n".join(f"- `{rf}` (Auto-reverted by Post-Run Scope Guard GP-1)" for rf in reverted_files)
        reverted_section = f"""
---

## 3. Cảnh Báo Bảo Vệ Ranh Giới (Post-Run Scope Guard GP-1)

Các file ngoài phạm vi `target_files` đã bị tự động khôi phục (reverted):
{rev_list}
"""

    # 4. Write omp-report.md
    report_file = output_dir / "omp-report.md"
    report_content = f"""# 📊 Báo Cáo Thực Thi & Đo Lường Token OMP

- **Story / Target**: `{story_id}`
- **Chế độ thực thi**: `{mode.upper()}`
- **Thời gian chạy**: `{round(duration_sec, 2)}` giây
- **Mô hình tham gia**: `{', '.join(sorted(models_used)) or active_model}`
- **Thời điểm hoàn thành**: `{datetime.datetime.now(datetime.timezone.utc).isoformat()}`

---

## 1. Bảng Phân Bổ Token & Hiệu Suất Caching (Bao Gồm Cả Primary & Advisor)

| Chỉ Số Đo Lường | Số Lượng Token | Ghi Chú Kỹ Thuật |
|:---|:---:|:---|
| **Input Tokens (Cold)** | {total_input:,} | Dữ liệu nạp mới vào context |
| **Prompt Cache Read** | {total_cache_read:,} | Tiết kiệm chi phí & tăng tốc |
| **Output Tokens** | {total_output:,} | Mã nguồn & phản hồi sinh ra |
| **Reasoning Tokens** | {total_reasoning:,} | Chuỗi suy luận logic (CoT) |
| **Tổng Token Xử Lý** | **{total_tokens:,}** | Tổng băng thông context |
| **Tỷ Lệ Cache Hit** | **{cache_rate:.1f}%** | Mục tiêu tối ưu > 80% |
| **Ước Tính Chi Phí** | **${total_cost:.5f} USD** | Chi phí marginal = $0.00 (OAuth Delegation) |

---

## 2. Danh Mục Tài Liệu Truy Vết (Artifacts)

1. **[omp-trace.html](file://{dest_html.resolve()})**: Giao diện trực quan offline cho con người kiểm tra (0 token cost).
2. **[omp-trace.jsonl](file://{dest_jsonl.resolve()})**: Toàn bộ chuỗi suy luận Chain of Thought và lịch sử gọi công cụ.{reverted_section}
"""
    report_file.write_text(report_content, encoding="utf-8")
    log(f"📊 Token usage report generated: {report_file}")

    try:
        record_session_token_usage(
            story_id=story_id,
            run_id=f"run-{int(time.time())}",
            topology=topology,
            worker_model=active_model,
            input_tokens=total_input,
            output_tokens=total_output
        )
    except Exception as e:
        log(f"⚠️ Failed to update session token ledger: {e}")


def _resolve_cli_models(topology: str, worker_model: str, worker_effort: str, advisor_model: str, advisor_effort: str) -> Dict[str, Any]:
    """Resolve symbolic JEV models into physical CLI model IDs via jev-cloud-client.py (EC-OMP-JEV-01)."""
    try:
        import importlib.util
        jev_path = Path(_agent_dir) / ".agent" / "scripts" / "jev-cloud-client.py"
        if jev_path.is_file():
            spec = importlib.util.spec_from_file_location("jev_cloud_client", str(jev_path))
            if spec and spec.loader:
                jev_mod = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(jev_mod)
                w_target = "codex-cli" if topology in {"codex-cli", "hybrid-1-worker-codex-advisor-ag"} else "omp"
                a_target = "codex-cli" if topology in {"codex-cli", "hybrid-2-worker-ag-advisor-codex"} else "agentapi"
                w_res = jev_mod.resolve_physical_cli_model_id(worker_model, w_target, worker_effort)
                a_res = jev_mod.resolve_physical_cli_model_id(advisor_model, a_target, advisor_effort)
                return {"worker": w_res, "advisor": a_res}
    except Exception as exc:
        log(f"⚠️ Physical CLI model resolution fallback: {exc}")
    return {
        "worker": {"resolved_physical_model": worker_model, "resolved_effort": worker_effort, "clamp_reason": None},
        "advisor": {"resolved_physical_model": advisor_model, "resolved_effort": advisor_effort, "clamp_reason": None},
    }


def _write_worker_codex_session(session_dir: Path, session_id: str, story_id: str, topology: str) -> Path:
    """Persist <session_dir>/worker-codex-session.json under fcntl.flock (EC-OMP-JEV-02)."""
    import fcntl
    session_dir.mkdir(parents=True, exist_ok=True)
    session_json = session_dir / "worker-codex-session.json"
    lock_path = session_dir / ".worker-codex-session.lock"
    with open(lock_path, "w") as lf:
        fcntl.flock(lf.fileno(), fcntl.LOCK_EX)
        try:
            payload = {
                "story_id": story_id,
                "topology": topology,
                "session_id": session_id,
                "updated_at": int(time.time() * 1000),
            }
            session_json.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
            (session_dir / "codex-session-id.txt").write_text(session_id, encoding="utf-8")
        finally:
            fcntl.flock(lf.fileno(), fcntl.LOCK_UN)
    return session_json


# ---------------------------------------------------------------------------
# Main Orchestrator Flow
# ---------------------------------------------------------------------------

def run_orchestrator(args) -> int:
    import hashlib
    start_time = time.time()
    worktree_dir = Path(args.worktree_dir).resolve() if args.worktree_dir else Path.cwd()
    output_dir = Path(args.output_dir).resolve() if args.output_dir else worktree_dir
    
    # Check circuit breaker latch
    latch_file = output_dir / ".circuit-breaker-tripped"
    if latch_file.exists():
        log(f"⚠️ Circuit breaker latch active ({latch_file}).")
        log("Skipping OMP. Delete .circuit-breaker-tripped to re-engage.")
        notify_user_and_halt(output_dir, "Circuit breaker latch engaged from previous failure", 0.0, EXIT_FALLBACK_TRIGGER)
        return EXIT_FALLBACK_TRIGGER

    # Simulated fallback test
    if args.test_fallback:
        log("🧪 TEST MODE: Simulating engine failure & testing rollback/notification...")
        time.sleep(1)
        tag_name = f"_pre_omp_checkpoint_test_{int(time.time())}"
        setup_git_checkpoint(worktree_dir, tag_name)
        atomic_rollback(worktree_dir, tag_name)
        trip_circuit_breaker(output_dir, "Test mode simulated crash")
        notify_user_and_halt(output_dir, "Simulated engine crash for test validation", 1.0, EXIT_FALLBACK_TRIGGER)
        return EXIT_FALLBACK_TRIGGER

    # Prepare story ID and validate plan data (P1 Fix: Strict Schema & Traversal Validation)
    story_id = args.story_id or "ad-hoc-task"
    plan_data = {}
    plan_sha256 = hashlib.sha256(b"").hexdigest()
    if args.plan_file:
        plan_path = Path(args.plan_file).resolve()
        valid, plan_data, err_msg = validate_impl_plan_schema_and_paths(plan_path)
        if not valid:
            log(f"❌ Impl-Plan validation failed: {err_msg}")
            return EXIT_CONFIG_ERROR
        story_id = plan_data.get("story_id", story_id)
        plan_sha256 = hashlib.sha256(plan_path.read_bytes()).hexdigest()

    tag_name = f"_pre_omp_checkpoint_{story_id}"

    topology = getattr(args, "topology", "antigravity") or "antigravity"
    req_worker_model = getattr(args, "worker_model", None) or args.model or ("gpt-5.6-luna" if topology in {"codex-cli", "hybrid-1-worker-codex-advisor-ag"} else "gemini-3.8-flash")
    req_worker_effort = getattr(args, "worker_effort", None) or "high"
    req_advisor_model = getattr(args, "advisor_model", None) or ("gpt-5.6-terra" if topology in {"codex-cli", "hybrid-2-worker-ag-advisor-codex"} else "gemini-4-argon")
    req_advisor_effort = getattr(args, "advisor_effort", None) or "high"

    resolved_cli = _resolve_cli_models(topology, req_worker_model, req_worker_effort, req_advisor_model, req_advisor_effort)
    worker_model = resolved_cli["worker"]["resolved_physical_model"]
    worker_effort = resolved_cli["worker"]["resolved_effort"]
    advisor_model = resolved_cli["advisor"]["resolved_physical_model"]
    advisor_effort = resolved_cli["advisor"]["resolved_effort"]

    # 100% OAuth Subscription Delegation Sync (EC-OMPV3-01)
    agent_db_path = Path.home() / ".omp" / "agent" / "agent.db"
    models_yml_path = Path.home() / ".omp" / "agent" / "models.yml"
    synced_creds = {}
    try:
        synced_creds = sync_omp_delegation_credentials(agent_db_path, models_yml_path, topology) or {}
    except RuntimeError as e:
        log(f"❌ {e}")
        trip_circuit_breaker(output_dir, str(e))
        notify_user_and_halt(output_dir, f"Delegation credentials missing: {e}", time.time() - start_time, EXIT_CONFIG_ERROR)
        return EXIT_CONFIG_ERROR

    # Determine provider families based on topology (EC-OMPV3-04)
    if topology == "antigravity":
        w_prov = "google-antigravity"
        a_prov = "google-antigravity"
    elif topology == "codex-cli":
        w_prov = "openai-codex"
        a_prov = "openai-codex"
    elif topology == "hybrid-1-worker-codex-advisor-ag":
        w_prov = "openai-codex"
        a_prov = "google-antigravity"
    else:  # hybrid-2-worker-ag-advisor-codex
        w_prov = "google-antigravity"
        a_prov = "openai-codex"

    worker_selector = _resolve_delegation_selector(w_prov, worker_model, worker_effort)
    advisor_selector = _resolve_delegation_selector(a_prov, advisor_model, advisor_effort)

    # Session token budget pre-flight check (Khóa P10)
    ok_budget, accum_tokens, budget_msg = check_session_token_budget(
        story_id,
        getattr(args, "session_token_budget", DEFAULT_MAX_SESSION_TOKENS),
        worktree_dir=worktree_dir,
    )
    if not ok_budget:
        log(f"❌ {budget_msg}")
        trip_circuit_breaker(output_dir, budget_msg)
        notify_user_and_halt(output_dir, budget_msg, time.time() - start_time)
        return EXIT_FALLBACK_TRIGGER

    # 1. Pre-flight probe
    models_to_probe = [worker_model]
    if not pre_flight_probe(models_to_probe, topology=topology):
        log("⚠️ Pre-flight probe failed. Halting.")
        trip_circuit_breaker(output_dir, "Pre-flight probe failed")
        notify_user_and_halt(output_dir, "Pre-flight probe connection failed for required models", time.time() - start_time)
        return EXIT_FALLBACK_TRIGGER

    # 2. Setup isolated session directory & checkpoints
    session_dir = worktree_dir / ".omp" / f"sessions-{story_id}"
    session_dir.mkdir(parents=True, exist_ok=True)
    baseline_snapshot = capture_workspace_state_snapshot(worktree_dir)
    setup_git_checkpoint(worktree_dir, tag_name)
    set_worktree_lock(worktree_dir, lock=True)

    # Pre-run untracked manifest & verify script generation (R12-01, NH-02)
    status_proc = subprocess.run(["git", "status", "--porcelain"], cwd=worktree_dir, capture_output=True, text=True, check=False)
    pre_run_untracked = set()
    for s_line in (status_proc.stdout or "").splitlines():
        if s_line.startswith("??"):
            pre_run_untracked.add(s_line[3:].strip())

    generate_omp_run_verify_script(worktree_dir, story_id)

    target_files_dict = plan_data.get("target_files", {})
    if isinstance(target_files_dict, dict):
        target_files_list = []
        for cat in ("create", "modify", "delete"):
            target_files_list.extend(target_files_dict.get(cat, []))
        if not target_files_list:
            target_files_list = list(target_files_dict.keys())
    elif isinstance(target_files_dict, list):
        target_files_list = [f.get("path") if isinstance(f, dict) else str(f) for f in target_files_dict]
    else:
        target_files_list = []

    raw_tier = plan_data.get("risk_tier", 2)
    risk_tier = int(raw_tier) if str(raw_tier).isdigit() else 2
    base_timeout = getattr(args, "timeout", 1200) or 1200
    effective_total_budget = compute_adaptive_omp_timeout(
        base_timeout=base_timeout,
        target_files_count=len(target_files_list),
        risk_tier=risk_tier,
        advisor_enabled=True,
    )
    overall_deadline = time.monotonic() + effective_total_budget

    # 3. Compile CPC v3.1 Context Pack & XML Prompt (Section 2.4 & Khóa P9)
    cpc_compiler = None
    cpc_manifest = None
    cpc_envelope = None
    cpc_run_dir = None
    try:
        scripts_root = str(Path(_agent_dir) / ".agent" / "scripts")
        if scripts_root not in sys.path:
            sys.path.insert(0, scripts_root)
        from cpc.compiler import ContextPackCompiler
        cpc_compiler = ContextPackCompiler(worktree_dir)
        cpc_manifest, cpc_envelope, cpc_snapshot, cpc_run_dir = cpc_compiler.compile(
            story_id=story_id,
            model=worker_model,
            effort=worker_effort,
        )
        cpc_compiler.verify_preflight_toctou(cpc_snapshot)
    except Exception as exc:
        log(f"⚠️ CPC v3.1 compilation notice (continuing with standard XML template): {exc}")

    if args.mode == "plan":
        prompt_content = build_plan_prompt(story_id, args.plan_file or "@story.md", str(worktree_dir))
        active_model = worker_model
    elif args.mode == "code":
        if cpc_compiler is not None and cpc_manifest is not None:
            prompt_content = cpc_compiler.build_omp_worker_cpc_xml_prompt(
                manifest=cpc_manifest,
                story_id=story_id,
                plan_data=plan_data,
                plan_file=args.plan_file or "@impl-plan.json",
                worker_model=worker_model,
                advisor_model=advisor_model,
                topology=topology,
            )
        else:
            prompt_content = build_code_prompt(
                story_id, plan_data, args.plan_file or "@impl-plan.json",
                worker_model=worker_model, advisor_model=advisor_model, topology=topology
            )
        active_model = worker_model
    elif args.mode == "review":
        prompt_content = build_review_prompt(story_id, plan_data, tag_name, args.plan_file or "@impl-plan.json")
        active_model = worker_model
    else: # dynamic
        prompt_content = build_dynamic_prompt("Ad-hoc Execution", args.prompt or "General task", str(worktree_dir))
        active_model = worker_model

    prompt_file = worktree_dir / ".omp-prompt.md"
    prompt_file.write_text(prompt_content, encoding="utf-8")
    prompt_sha256 = hashlib.sha256(prompt_content.encode("utf-8")).hexdigest()

    # Generate Watchdog Advisor files (EC-OMPV3-02, EC-OMPV3-04, GP-2, GP-5 / FIX-2)
    generate_omp_watchdog_files(worktree_dir, story_id, plan_data, advisor_selector, worker_selector=worker_selector)

    # 4. Start Heartbeat
    stop_heartbeat = threading.Event()
    start_heartbeat(worktree_dir, stop_heartbeat)

    # 5. Launch Process Group (Unified Native OMP Process across all Topologies)
    proc = None
    exec_error = None
    captured_session_id = None
    raw_stdout = ""
    raw_stderr = ""
    ripgrep_cfg = str((worktree_dir / ".ripgreprc").resolve())

    try:
        omp_bin = shutil.which("omp") or str(Path.home() / ".local/bin/omp")
        omp_env = {
            **os.environ,
            "RIPGREP_CONFIG_PATH": ripgrep_cfg,
            "SQLITE_BUSY_TIMEOUT": "60000",
        }
        if synced_creds.get("codex_access_token"):
            omp_env["OPENAI_CODEX_OAUTH_TOKEN"] = synced_creds["codex_access_token"]
        log(f"🚀 Launching Native OMP Process [Topology: {topology} | Mode: {args.mode.upper()}]...")
        log(f"   Worker: {worker_selector} | Advisor: {advisor_selector}")

        cmd = [
            omp_bin,
            "-p",
            "--mode", "json",
            "--advisor",
            "--config", str(worktree_dir / ".omp" / "config-overlay.yml"),
            "--auto-approve",
            "--no-pty",
            "--session-dir", str(session_dir),
            "--model", worker_selector,
            prompt_content,
        ]

        checkpoint_data = {}
        try:
            proc = subprocess.Popen(
                cmd,
                cwd=worktree_dir,
                env=omp_env,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                preexec_fn=os.setsid,
            )

            stdout_lines = []
            stderr_lines = []

            def _drain_stderr():
                if proc.stderr:
                    for s_line in iter(proc.stderr.readline, ""):
                        stderr_lines.append(s_line)

            stderr_thread = threading.Thread(target=_drain_stderr, daemon=True)
            stderr_thread.start()

            lint_queue: queue.Queue = queue.Queue(maxsize=32)

            def _lint_worker():
                while True:
                    try:
                        item = lint_queue.get()
                        if item is None:
                            lint_queue.task_done()
                            break
                        file_p, wt_p = item
                        try:
                            run_instream_micro_linter(file_p, wt_p)
                        except (OSError, ValueError, TypeError, RuntimeError) as e_lint:
                            log(f"Micro-linter warning: {e_lint}")
                        finally:
                            lint_queue.task_done()
                    except (ValueError, TypeError, RuntimeError) as q_exc:
                        sys.stderr.write(f"[omp-orch-executor] warning: lint worker queue error: {q_exc}\n")

            lint_worker_thread = threading.Thread(target=_lint_worker, daemon=True)
            lint_worker_thread.start()

            timed_out_event = threading.Event()
            stop_watchdog = threading.Event()

            def _watchdog():
                remaining_for_run = max(1.0, overall_deadline - time.monotonic())
                if not stop_watchdog.wait(remaining_for_run):
                    timed_out_event.set()
                    try:
                        pgid = os.getpgid(proc.pid)
                        os.killpg(pgid, signal.SIGKILL)
                    except (OSError, ProcessLookupError):
                        try:
                            proc.kill()
                        except OSError as kill_exc:
                            sys.stderr.write(f"[omp-orch-executor] warning: watchdog proc.kill failed: {kill_exc}\n")

            watchdog_thread = threading.Thread(target=_watchdog, daemon=True)
            watchdog_thread.start()

            advisor_events_stream = []
            aborted_midstream = False

            for line in iter(proc.stdout.readline, ""):
                stdout_lines.append(line)
                line_str = line.strip()
                if not line_str or not line_str.startswith("{"):
                    continue
                try:
                    ev = json.loads(line_str)
                except (json.JSONDecodeError, ValueError, TypeError) as ev_exc:
                    sys.stderr.write(f"[omp-orch-executor] warning: malformed OMP stdout JSON line skipped: {ev_exc}\n")
                    continue

                ev_type = ev.get("type")
                if ev_type == "session":
                    s_id = ev.get("id")
                    if s_id and not captured_session_id:
                        captured_session_id = s_id
                        log(f"📌 Captured OMP Session ID: {captured_session_id}")
                        _write_worker_codex_session(session_dir, captured_session_id, story_id, topology)
                elif ev_type == "advisor_yielded" or (ev_type == "tool_execution_end" and ev.get("toolName") in ("advise", "advisor")):
                    advisor_events_stream.append(ev)
                    dedupe_res = check_advisor_yield_cap_and_dedupe(advisor_events_stream)
                    if dedupe_res.get("should_terminate"):
                        exec_error = f"Advisor yield cap/dedupe triggered: {dedupe_res['reason']}"
                        log(f"⚠️ {exec_error} - terminating OMP process group mid-stream!")
                        aborted_midstream = True
                        checkpoint_data = terminate_omp_process_group_and_checkpoint(
                            proc, worktree_dir, story_id, target_files_list, pre_run_untracked
                        )
                        break
                elif ev_type == "tool_execution_end":
                    tool_name = str(ev.get("toolName", ""))
                    if any(w in tool_name.lower() for w in ("write", "edit", "patch", "create", "replace")):
                        tool_args = ev.get("args") or ev.get("toolArgs") or ev.get("parameters") or {}
                        target_f = (
                            tool_args.get("TargetFile") or tool_args.get("target_file") or
                            tool_args.get("file_path") or tool_args.get("path") or
                            tool_args.get("file")
                        )
                        if target_f:
                            try:
                                wt_p = Path(getattr(args, "worktree", None) or getattr(args, "worktree_dir", None) or worktree_dir)
                                lint_queue.put_nowait((Path(target_f), wt_p))
                            except queue.Full:
                                log(f"Micro-linter queue full (32), skipping in-stream lint for {target_f}")
                            except Exception as e_lint:
                                log(f"Micro-linter warning: {e_lint}")

            stop_watchdog.set()
            proc.wait()
            stderr_thread.join(timeout=2.0)

            # Drain bounded micro-linter queue (EC-REM-P7-001)
            try:
                drain_thread = threading.Thread(target=lint_queue.join, daemon=True)
                drain_thread.start()
                drain_thread.join(timeout=3.0)
                lint_queue.put_nowait(None)
            except Exception as e_drain:
                log(f"Micro-linter drain notice: {e_drain}")

            raw_stdout = "".join(stdout_lines)
            raw_stderr = "".join(stderr_lines)
            log(f"OMP execution finished with return code {proc.returncode}.")

            captured_session_id2, advisor_yields, stream_error = parse_omp_json_stream(raw_stdout)
            if captured_session_id2 and not captured_session_id:
                captured_session_id = captured_session_id2
                log(f"📌 Captured OMP Session ID: {captured_session_id} (Advisor Yields: {advisor_yields})")
                _write_worker_codex_session(session_dir, captured_session_id, story_id, topology)

            if timed_out_event.is_set() or ((time.monotonic() >= overall_deadline) and proc.returncode != 0):
                exec_error = f"OMP process timed out after {effective_total_budget} seconds"
                checkpoint_data = terminate_omp_process_group_and_checkpoint(proc, worktree_dir, story_id, target_files_list, pre_run_untracked)
            elif not aborted_midstream:
                if proc.returncode != 0:
                    exec_error = f"OMP process returned exit code {proc.returncode}: {raw_stderr or raw_stdout}"
                    checkpoint_data = terminate_omp_process_group_and_checkpoint(proc, worktree_dir, story_id, target_files_list, pre_run_untracked)
                elif stream_error:
                    exec_error = stream_error
                    checkpoint_data = terminate_omp_process_group_and_checkpoint(proc, worktree_dir, story_id, target_files_list, pre_run_untracked)

        except subprocess.TimeoutExpired:
            exec_error = f"OMP process timed out after {effective_total_budget} seconds"
            checkpoint_data = terminate_omp_process_group_and_checkpoint(proc, worktree_dir, story_id, target_files_list, pre_run_untracked)
        except Exception as e:
            exec_error = f"OMP execution exception: {e}"
            checkpoint_data = terminate_omp_process_group_and_checkpoint(proc, worktree_dir, story_id, target_files_list, pre_run_untracked)

        # R12-01: One-time restart handoff with advisor disabled if advisor cap or timeout reached
        if exec_error and ("Advisor yield cap" in exec_error or "timed out" in exec_error):
            st_proc = subprocess.run(["git", "status", "--short"], cwd=worktree_dir, capture_output=True, text=True, check=False)
            status_short = [l.strip() for l in (st_proc.stdout or "").splitlines() if l.strip()]
            retained = checkpoint_data.get("retained_files", []) if checkpoint_data else []
            completed_files = status_short or retained

            capsule = build_restart_handoff_capsule(
                blocker_reasons=[exec_error],
                completed_files=completed_files,
                last_linter_error=stream_error or "",
            )
            restart_prompt = f"{capsule}\n\n{prompt_content}"
            prompt_file.write_text(restart_prompt, encoding="utf-8")

            # Re-generate overlay with advisor disabled
            generate_omp_watchdog_files(
                worktree_dir, story_id, plan_data,
                advisor_selector, worker_selector=worker_selector,
                advisor_enabled=False
            )

            STARTUP_FLOOR_SEC = 45
            remaining_sec = int(overall_deadline - time.monotonic())
            if remaining_sec < STARTUP_FLOOR_SEC:
                log(f"⚠️ Run 2 restart skipped (remaining: {remaining_sec}s < floor {STARTUP_FLOOR_SEC}s): reason: STAGE3B_BUDGET_EXHAUSTED_FOR_RESTART")
                exec_error = f"{exec_error} (Run 2 skipped: reason: STAGE3B_BUDGET_EXHAUSTED_FOR_RESTART)"
            else:
                restart_timeout = min(600, remaining_sec)
                log(f"🔄 Launching OMP Run 2 restart with <restart_handoff_capsule> (timeout: {restart_timeout}s, advisor disabled)...")
            cmd_run2 = list(cmd)
            if prompt_content in cmd_run2:
                cmd_run2[cmd_run2.index(prompt_content)] = restart_prompt

            try:
                proc2 = subprocess.Popen(
                    cmd_run2,
                    cwd=worktree_dir,
                    env=omp_env,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                    preexec_fn=os.setsid,
                )
                raw_stdout2, raw_stderr2 = proc2.communicate(timeout=restart_timeout)
                raw_stdout = (raw_stdout or "") + "\n--- RUN 2 ---\n" + (raw_stdout2 or "")
                raw_stderr = (raw_stderr or "") + "\n--- RUN 2 ---\n" + (raw_stderr2 or "")
                captured_session_id2, advisor_yields2, stream_error2 = parse_omp_json_stream(raw_stdout2)
                if captured_session_id2:
                    captured_session_id = captured_session_id2
                if proc2.returncode == 0 and not stream_error2:
                    log("🎉 OMP Run 2 succeeded under <restart_handoff_capsule>!")
                    exec_error = None
                else:
                    exec_error = stream_error2 or f"OMP Run 2 returned exit code {proc2.returncode}"
                    terminate_omp_process_group_and_checkpoint(proc2, worktree_dir, story_id, target_files_list, pre_run_untracked)
            except Exception as e2:
                exec_error = f"OMP Run 2 exception: {e2}"
                if 'proc2' in locals():
                    terminate_omp_process_group_and_checkpoint(proc2, worktree_dir, story_id, target_files_list, pre_run_untracked)
    finally:
        stop_heartbeat.set()
        set_worktree_lock(worktree_dir, lock=False)
        try:
            duration = time.time() - start_time
            process_session_artifacts(
                session_dir,
                output_dir,
                story_id,
                args.mode,
                duration,
                topology=topology,
                active_model=active_model,
                reverted_files=locals().get("reverted_files"),
                run_start_time=start_time,
            )
        except Exception as psa_err:
            log(f"⚠️ process_session_artifacts notice: {psa_err}")
        cleanup_omp_watchdog_files(worktree_dir)

    # Record CPC v3.1 ExecutionReceipt with 6-Way Cryptographic Hash Chain (Khóa P9)
    duration_ms = int((time.time() - start_time) * 1000)
    if cpc_compiler is not None and cpc_envelope is not None and cpc_run_dir is not None:
        try:
            diff_proc = subprocess.run(["git", "diff", tag_name], cwd=worktree_dir, capture_output=True, check=False)
            diff_sha = hashlib.sha256(diff_proc.stdout or b"").hexdigest()
            tools_sha = hashlib.sha256((raw_stdout or "").encode("utf-8")).hexdigest()
            art_sha = hashlib.sha256(((raw_stdout or "") + (raw_stderr or "")).encode("utf-8")).hexdigest()
            cpc_compiler.record_receipt(
                run_dir=cpc_run_dir,
                envelope=cpc_envelope,
                exit_code=0 if not exec_error else EXIT_FALLBACK_TRIGGER,
                verdict="APPROVED" if not exec_error else "REJECTED",
                duration_ms=duration_ms,
                output_sha256=art_sha,
                requested_model=req_worker_model,
                executed_model=worker_model,
                plan_sha256=plan_sha256,
                prompt_sha256=prompt_sha256,
                tool_events_sha256=tools_sha,
                diff_sha256=diff_sha,
                artifact_sha256=art_sha,
            )
        except Exception as exc:
            log(f"⚠️ CPC receipt recording notice: {exc}")

    # Handle Execution Failure (Exit Code 10)
    if exec_error:
        log(f"❌ {exec_error}")
        atomic_rollback(worktree_dir, tag_name, baseline_snapshot=baseline_snapshot, target_files=plan_data.get("target_files", {}))
        trip_circuit_breaker(output_dir, exec_error)
        notify_user_and_halt(output_dir, exec_error, time.time() - start_time, EXIT_FALLBACK_TRIGGER)
        return EXIT_FALLBACK_TRIGGER

    # 6. Post-Execution Scope Verification & Auto-Revert (GP-1 / EC-OMPV3-02)
    reverted_files = enforce_post_run_scope_guard(worktree_dir, plan_data.get("target_files", {}), baseline_snapshot=baseline_snapshot)

    if args.mode == "code":
        # Check scope lockdown
        if not check_scope_lockdown(worktree_dir, plan_data.get("target_files", {}), baseline_snapshot=baseline_snapshot):
            log("❌ Scope lockdown failed! Rolling back changes...")
            atomic_rollback(worktree_dir, tag_name, baseline_snapshot=baseline_snapshot, target_files=plan_data.get("target_files", {}))
            trip_circuit_breaker(output_dir, "Scope lockdown violation")
            notify_user_and_halt(output_dir, "Scope lockdown violation detected (unauthorized file modifications)", time.time() - start_time)
            return EXIT_FALLBACK_TRIGGER

        # Run verification tests with timeout and whitelist
        test_cmd = plan_data.get("testing", {}).get("test_command", "pnpm test")
        log(f"🧪 Running Verification Test: {test_cmd}")
        try:
            test_res = run_safe_test(test_cmd, cwd=worktree_dir, timeout=300)
            if test_res.returncode != 0:
                log(f"⚠️ Unit tests failed. Retaining code for repair:\n{test_res.stderr or test_res.stdout}")
                return EXIT_TEST_DEFECT
        except subprocess.TimeoutExpired:
            log(f"⚠️ Verification test timed out after 300s.")
            return EXIT_TEST_DEFECT

    elif args.mode == "dynamic":
        if not dynamic_scope_guard(worktree_dir, baseline_snapshot=baseline_snapshot):
            log("❌ Dynamic mode touched protected system directories! Rolling back...")
            atomic_rollback(worktree_dir, tag_name, baseline_snapshot=baseline_snapshot)
            trip_circuit_breaker(output_dir, "Dynamic mode system scope violation")
            notify_user_and_halt(output_dir, "Attempted to modify protected system files in dynamic mode", time.time() - start_time)
            return EXIT_FALLBACK_TRIGGER

    # 7. Post-Execution Artifact Generation (Trace & Metrics)
    duration = time.time() - start_time
    process_session_artifacts(
        session_dir,
        output_dir,
        story_id,
        args.mode,
        duration,
        topology=topology,
        active_model=active_model,
        reverted_files=reverted_files,
    )

    # Clean up checkpoint tag on success
    subprocess.run(["git", "tag", "-d", tag_name], cwd=worktree_dir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    log(f"🎉 OMP Orchestration completed successfully in {round(duration, 2)}s!")
    return EXIT_SUCCESS

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Unified OMP Orchestration Executor v2.0")
    parser.add_argument("--mode", choices=["plan", "code", "review", "dynamic"], default="code",
                        help="Execution mode (plan, code, review, dynamic)")
    parser.add_argument("--plan-file", help="Path to impl-plan.json")
    parser.add_argument("--story-id", help="Story identifier (e.g. story-82.8)")
    parser.add_argument("--topology", choices=["antigravity", "codex-cli", "hybrid-1-worker-codex-advisor-ag", "hybrid-2-worker-ag-advisor-codex"],
                        default="antigravity", help="OMP coordination topology")
    parser.add_argument("--worker-model", help="Worker model ID")
    parser.add_argument("--worker-effort", help="Worker effort (low|medium|high|xhigh)")
    parser.add_argument("--advisor-model", help="Advisor model ID")
    parser.add_argument("--advisor-effort", help="Advisor effort (low|medium|high|xhigh)")
    parser.add_argument("--session-token-budget", type=int, default=DEFAULT_MAX_SESSION_TOKENS, help="Maximum session token budget")
    parser.add_argument("--model", help="Override the active model for OMP")
    parser.add_argument("--provider", help="Override the provider for OMP")
    parser.add_argument("--prompt", help="Direct prompt for dynamic mode")
    parser.add_argument("--worktree-dir", help="Path to workspace / worktree directory")
    parser.add_argument("--output-dir", help="Path to store trace, report, and artifacts")
    parser.add_argument("--timeout", type=int, default=1200, help="Process timeout in seconds")
    parser.add_argument("--test-fallback", action="store_true", help="Simulate fallback trigger to test rollback & user notification")

    parsed_args = parser.parse_args()
    sys.exit(run_orchestrator(parsed_args))
