#!/usr/bin/env python3
"""
QA Sandbox with Watchmen HMAC Nonce Handshake & Story-Scoped Evidence Isolation (EC-HARNESS-P3-001).
"""
import json
import os
import sys
import glob
import secrets
import subprocess
import time
import socket
import argparse
import re as _re
import shutil
from pathlib import Path

_script_dir = Path(__file__).resolve().parent
_agent_scripts_dir = (_script_dir.parent / ".agent" / "scripts").resolve()
if str(_agent_scripts_dir) not in sys.path:
    sys.path.insert(0, str(_agent_scripts_dir))

from runtime_paths import resolve_project_root, normalize_story_id

def get_free_port():
    port_mgr = (_agent_scripts_dir / "worktree-port-manager.py")
    if port_mgr.is_file():
        try:
            res = subprocess.run(
                [sys.executable, str(port_mgr), "assign", "--json"],
                capture_output=True,
                text=True,
                timeout=5,
            )
            if res.returncode == 0:
                data = json.loads(res.stdout)
                if isinstance(data.get("port"), int):
                    return data["port"]
        except (subprocess.SubprocessError, OSError, json.JSONDecodeError, ValueError, TypeError) as exc:
            sys.stderr.write(f"[run-qa-sandbox] port manager lookup failed: {exc}\n")
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(('127.0.0.1', 0))
    port = s.getsockname()[1]
    s.close()
    return port

def parse_args():
    parser = argparse.ArgumentParser(description="QA Sandbox with Watchmen HMAC Nonce Handshake")
    parser.add_argument("--story-id", help="Story ID to target E2E test", default=None)
    parser.add_argument("--port", type=int, help="Dev server port", default=None)
    parser.add_argument("--cwd", help="Explicit project/worktree root", default=None)
    return parser.parse_known_args()

def main():
    args, unknown = parse_args()
    project_root = resolve_project_root(explicit_root=args.cwd)
    os.chdir(str(project_root))

    print("🔒 Starting QA Sandbox with Watchmen HMAC Nonce Handshake...")
    target_story_raw = args.story_id or os.environ.get('TARGET_STORY') or os.environ.get('STORY_ID') or ''
    clean_story = _re.sub(r'[^a-zA-Z0-9._-]', '', normalize_story_id(target_story_raw))

    scratch_dir = project_root / '_iwish-output' / 'adhoc-workspace' / 'scratch'
    scratch_dir.mkdir(parents=True, exist_ok=True)

    # Resolve port first to check if a server is already running with an existing valid nonce
    port = args.port or (int(os.environ['TARGET_PORT']) if 'TARGET_PORT' in os.environ else None) or (int(os.environ['PORT']) if 'PORT' in os.environ else None)

    server_already_running = False
    if port:
        try:
            with socket.create_connection(('127.0.0.1', port), timeout=1):
                server_already_running = True
        except OSError:
            server_already_running = False

    scoped_path = scratch_dir / f'.watchmen-nonce-{clean_story}' if clean_story else scratch_dir / '.watchmen-nonce'
    legacy_path = scratch_dir / '.watchmen-nonce'

    # EC-HARNESS-P3-001: If reusing an already-running dev server on explicit port, reuse its active nonce if present;
    # otherwise allocate a dedicated fresh port so our freshly generated VITE_QA_NONCE is guaranteed to match the Vite process!
    if server_already_running and (scoped_path.is_file() or legacy_path.is_file()):
        existing_nonce_file = scoped_path if scoped_path.is_file() else legacy_path
        nonce = existing_nonce_file.read_text(encoding='utf-8').strip()
        print(f"ℹ️ Reusing active dev server on port {port} with existing nonce {nonce[:8]}...")
    else:
        if server_already_running:
            # Port is occupied by a server with unknown nonce -> allocate an isolated free port for this story run
            port = get_free_port()
            server_already_running = False
        elif not port:
            port = get_free_port()
        nonce = secrets.token_hex(32)
        print(f"🔑 Generated Ephemeral WATCHMEN_NONCE: {nonce[:8]}... on isolated port {port}")

    os.environ['NEXT_PUBLIC_WATCHMEN_NONCE'] = nonce
    os.environ['WATCHMEN_NONCE'] = nonce
    os.environ['VITE_WATCHMEN_NONCE'] = nonce
    os.environ['VITE_QA_NONCE'] = nonce

    if clean_story:
        scoped_tmp = scratch_dir / f'.watchmen-nonce-{clean_story}.{os.getpid()}.tmp'
        scoped_tmp.write_text(nonce, encoding='utf-8')
        os.replace(scoped_tmp, scoped_path)
    legacy_tmp = scratch_dir / f'.watchmen-nonce.{os.getpid()}.tmp'
    legacy_tmp.write_text(nonce, encoding='utf-8')
    os.replace(legacy_tmp, legacy_path)

    server_process = None
    api_process = None
    api_port = int(os.environ.get('API_PORT', '3212'))
    api_already_running = False
    try:
        with socket.create_connection(('127.0.0.1', api_port), timeout=1):
            api_already_running = True
    except OSError:
        api_already_running = False

    try:
        from lib.project_graph_config import resolve_project_graph_config

        stack_adapter = resolve_project_graph_config(project_root).stack_adapter
    except (ImportError, OSError, ValueError) as exc:
        sys.stderr.write(f"[run-qa-sandbox] stack_adapter config fallback to fastify-vite-spa: {exc}\n")
        stack_adapter = "fastify-vite-spa"

    pkg_scripts = {}
    pkg_json_path = Path(project_root) / 'package.json'
    if pkg_json_path.is_file():
        try:
            pkg_scripts = (json.loads(pkg_json_path.read_text(encoding='utf-8')).get('scripts') or {})
        except (OSError, json.JSONDecodeError, ValueError, TypeError, AttributeError) as exc:
            sys.stderr.write(f"[run-qa-sandbox] failed reading {pkg_json_path}: {exc}\n")
            pkg_scripts = {}

    has_fastify_entry = (Path(project_root) / 'src' / 'backend' / 'server.ts').is_file()
    if stack_adapter == 'fastify-vite-spa' or (has_fastify_entry and 'dev:api' not in pkg_scripts):
        be_cmd = ['npx', 'tsx', 'src/backend/server.ts']
    elif 'dev:api' in pkg_scripts:
        be_cmd = ['npm', 'run', 'dev:api']
    elif 'server' in pkg_scripts:
        be_cmd = ['npm', 'run', 'server']
    else:
        be_cmd = None
        api_already_running = True
        api_port = port

    if not api_already_running and be_cmd is not None:
        api_port = get_free_port()
        os.environ['API_PORT'] = str(api_port)
        os.environ.setdefault('REDIS_CACHE_URL', 'redis://127.0.0.1:6383')
        os.environ.setdefault('REDIS_QUEUE_URL', 'redis://127.0.0.1:6381')
        os.environ.setdefault('REDIS_SESSION_URL', 'redis://127.0.0.1:6381')
        os.environ.setdefault('VECTOR_REDIS_URL', 'redis://127.0.0.1:6382')
        os.environ.setdefault('FALKORDB_URL', 'redis://127.0.0.1:6380')
        api_env = os.environ.copy()
        api_env['API_PORT'] = str(api_port)
        api_env['PORT'] = str(api_port)
        api_env['REDIS_CACHE_URL'] = os.environ.get('REDIS_CACHE_URL', 'redis://127.0.0.1:6383')
        api_env['REDIS_QUEUE_URL'] = os.environ.get('REDIS_QUEUE_URL', 'redis://127.0.0.1:6381')
        api_env['REDIS_SESSION_URL'] = os.environ.get('REDIS_SESSION_URL', 'redis://127.0.0.1:6381')
        api_env['VECTOR_REDIS_URL'] = os.environ.get('VECTOR_REDIS_URL', 'redis://127.0.0.1:6382')
        api_env['FALKORDB_URL'] = os.environ.get('FALKORDB_URL', 'redis://127.0.0.1:6380')
        print(f"🚀 Starting Backend server ({stack_adapter}) on port {api_port}...")
        api_process = subprocess.Popen(
            be_cmd,
            cwd=str(project_root),
            env=api_env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL
        )
        backend_ready = False
        last_backend_err = None
        for _ in range(30):
            time.sleep(1)
            try:
                with socket.create_connection(('127.0.0.1', api_port), timeout=1):
                    print(f"✅ Backend ready on port {api_port}.")
                    backend_ready = True
                    break
            except OSError as exc:
                last_backend_err = exc
        if not backend_ready and last_backend_err is not None:
            sys.stderr.write(
                f"[run-qa-sandbox] backend readiness check timed out on port {api_port}: {last_backend_err}\n"
            )

    if not server_already_running:
        dev_cmd = (
            ['npx', 'vite', '--port', str(port), '--host', '127.0.0.1']
            if stack_adapter == 'fastify-vite-spa'
            else ['npx', 'next', 'dev', '-p', str(port), '-H', '127.0.0.1']
        )
        print(f"🚀 Starting Frontend dev server ({stack_adapter}) on port {port} (proxying /api -> {api_port})...")
        server_process = subprocess.Popen(
            dev_cmd,
            cwd=str(project_root),
            env=os.environ.copy(),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL
        )
    
    try:
        if not server_already_running:
            print("⏳ Waiting for dev server to be ready...")
            ready = False
            last_frontend_err = None
            for _ in range(30):
                time.sleep(1)
                try:
                    with socket.create_connection(('127.0.0.1', port), timeout=1):
                        ready = True
                        break
                except OSError as exc:
                    last_frontend_err = exc
            
            if not ready:
                if last_frontend_err is not None:
                    sys.stderr.write(
                        f"[run-qa-sandbox] frontend readiness check failed on port {port}: {last_frontend_err}\n"
                    )
                print("❌ Dev server failed to start in time.")
                sys.exit(1)
            
        print("✅ Dev server ready. Executing Playwright...")
        
        target_test = None
        if len(unknown) >= 1 and os.path.exists(unknown[0]):
            target_test = unknown[0]
        elif clean_story:
            matching_files = (
                glob.glob(f"tests/e2e/**/Story-{clean_story}.spec.ts", recursive=True) +
                glob.glob(f"tests/e2e/**/story-{clean_story}.spec.ts", recursive=True) +
                glob.glob(f"tests/e2e/Story-{clean_story}.spec.ts")
            )
            if matching_files:
                target_test = matching_files[0]
                print(f"🎯 Targeting E2E test file: {target_test}")
            else:
                print(f"❌ FAIL: No specific test file found for Story {clean_story}. Cannot run sandbox.")
                sys.exit(1)
        else:
            spec_locks = glob.glob(".agent/cache/spec-locks/story-*.json")
            if spec_locks:
                latest_lock = max(spec_locks, key=os.path.getmtime)
                m = _re.search(r"story-([a-zA-Z0-9\.]+)\.json", os.path.basename(latest_lock))
                if m:
                    sid = m.group(1)
                    for root_w, _, files in os.walk("tests/e2e"):
                        for f in files:
                            if f.lower() == f"story-{sid}.spec.ts":
                                target_test = os.path.join(root_w, f)
                                break
        
        pw_env = os.environ.copy()
        pw_env['TARGET_PORT'] = str(port)
        pw_env['WATCHMEN_NONCE'] = nonce
        pw_env['NEXT_PUBLIC_WATCHMEN_NONCE'] = nonce
        pw_env['VITE_WATCHMEN_NONCE'] = nonce
        pw_env['VITE_QA_NONCE'] = nonce
        
        cmd = ['npx', 'playwright', 'test']
        if os.path.exists('playwright.ui.config.ts'):
            cmd.extend(['-c', 'playwright.ui.config.ts'])
        if target_test:
            print(f"🎯 Targeting story spec file: {target_test}")
            cmd.append(target_test)

        pw_process = subprocess.run(cmd, cwd=str(project_root), env=pw_env)
        
        if pw_process.returncode != 0:
            print("❌ Playwright tests failed.")
            sys.exit(pw_process.returncode)
            
        print("✅ Playwright execution completed successfully.")
        
        # EC-HARNESS-P3-001: Strictly filter QA evidence propagation by clean_story so parallel stories never contaminate each other
        for sb_ev in glob.glob(str(project_root / '.worktrees/sandbox-*/_iwish-output/**/qa/evidence'), recursive=True):
            if clean_story and f"Story-{clean_story}" not in sb_ev and f"story-{clean_story}" not in sb_ev:
                continue
            if os.path.isdir(sb_ev):
                prefix = sb_ev.split('_iwish-output')[0]
                rel_path = os.path.relpath(sb_ev, prefix)
                root_ev = str(project_root / rel_path)
                if os.path.isdir(root_ev):
                    print(f"📦 Synchronizing story-scoped QA evidence ({clean_story or 'all'}) to sandbox: {sb_ev}")
                    for ef in os.listdir(root_ev):
                        src_f = os.path.join(root_ev, ef)
                        if os.path.isfile(src_f):
                            shutil.copy2(src_f, os.path.join(sb_ev, ef))
        
    finally:
        if server_process is not None:
            print("🧹 Terminating Vite dev server...")
            server_process.terminate()
            try:
                server_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server_process.kill()
        if api_process is not None:
            print("🧹 Terminating Fastify Backend server...")
            api_process.terminate()
            try:
                api_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                api_process.kill()

if __name__ == "__main__":
    main()
