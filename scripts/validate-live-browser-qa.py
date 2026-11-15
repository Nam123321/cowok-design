#!/usr/bin/env python3
import os
import sys
import re
import json
import glob
import urllib.request
import urllib.error
import hashlib
import time

def parse_args():
    import argparse
    parser = argparse.ArgumentParser(description="Validate Live Browser QA testing against static preview cheats.")
    parser.add_argument("epic_id", help="Epic ID (e.g. 01, 58)")
    parser.add_argument("story_id", help="Story ID (e.g. 01.16, 58.5)")
    parser.add_argument("--story-dir", help="Explicit story directory path", default=None)
    parser.add_argument("--check-liveness", action="store_true", help="Probe dev server port liveness")
    parser.add_argument("--port", type=int, help="Target port to probe", default=None)
    return parser.parse_args()

def resolve_story_dir(epic_id, story_id, explicit_dir=None):
    if explicit_dir and os.path.exists(explicit_dir):
        return explicit_dir
    patterns = [
        f"_iwish-output/3. Development/1. Epic & Story/*/Epic-{epic_id}/Story-{story_id}",
        f"_iwish-output/3. Development/1. Epic & Story/*/Epic-{epic_id}/Story-{story_id}.*",
        f"_iwish-output/stories/story-{story_id}",
    ]
    for pat in patterns:
        matches = glob.glob(pat)
        if matches:
            return matches[0]
    return None

def resolve_test_files(epic_id, story_id, story_dir):
    test_files = set()
    # Check traceability.json
    if story_dir:
        trace_path = os.path.join(story_dir, "traceability.json")
        if os.path.exists(trace_path):
            try:
                with open(trace_path, "r", encoding="utf-8") as f:
                    data = json.load(f)
                for entry in data.get("traceability_matrix", []):
                    for t in entry.get("tests", []):
                        if t.endswith((".spec.ts", ".spec.js", ".test.ts", ".test.js")):
                            if os.path.exists(t):
                                test_files.add(t)
            except (OSError, json.JSONDecodeError, ValueError, TypeError, AttributeError) as exc:
                sys.stderr.write(f"[validate-live-browser-qa] failed reading {trace_path}: {exc}\n")

    # Glob fallback
    patterns = [
        f"tests/e2e/**/Story-{story_id}*.*",
        f"tests/e2e/**/story-{story_id}*.*",
        f"tests/e2e/**/Epic-{epic_id}/**/*{story_id}*.*",
        f"tests/e2e/**/epic-{epic_id}/**/*{story_id}*.*",
        f"tests/e2e/Story-{story_id}/*.*",
        f"tests/e2e/story-{story_id}/*.*",
    ]
    for pat in patterns:
        for f in glob.glob(pat, recursive=True):
            if f.endswith((".spec.ts", ".spec.js", ".ts", ".js")):
                test_files.add(f)
    return list(test_files)

def check_script_origins(test_files):
    print("🔍 [Anti-Preview Guard 1/5] Auditing test script navigation origins...")
    if not test_files:
        print("⚠️ Warning: No explicit test script files resolved for origin audit.")
        return True, None

    forbidden_patterns = [
        (r'page\.goto\s*\(\s*[\'"`]file:\/\/', "Forbidden protocol: file:// is strictly prohibited."),
        (r'page\.goto\s*\(\s*[\'"`][^\'"`]*preview\.html', "Forbidden navigation: preview.html is a static design mockup, not live app."),
        (r'page\.setContent\s*\(', "Forbidden technique: page.setContent() injects raw HTML instead of browsing live app."),
        (r'goto\s*\(\s*[\'"`]\.\.?\/', "Forbidden navigation: Relative local file path used in page.goto()."),
        (r'document\.createElement\s*\(', "Forbidden DOM synthesis: document.createElement() inside Playwright tests is strictly prohibited."),
        (r'\.innerHTML\s*=', "Forbidden DOM synthesis: .innerHTML assignment inside Playwright tests is strictly prohibited."),
        (r'document\.body\.appendChild\s*\(', "Forbidden DOM synthesis: document.body.appendChild() inside Playwright tests is strictly prohibited."),
        (r'insertAdjacentHTML\s*\(', "Forbidden DOM synthesis: insertAdjacentHTML() inside Playwright tests is strictly prohibited."),
    ]

    valid_origin_pattern = r'page\.goto\s*\(\s*[\'"`](https?:\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0)(?::\d+)?(?:\/[^\'"`]*)?)[\'"`]'
    extracted_ports = set()
    violations = []

    for tf in test_files:
        with open(tf, "r", encoding="utf-8", errors="ignore") as f:
            content = f.read()

        for pat, reason in forbidden_patterns:
            if re.search(pat, content):
                violations.append(f"{os.path.basename(tf)}: {reason}")

        matches = re.findall(valid_origin_pattern, content)
        for url in matches:
            port_match = re.search(r':(\d+)', url)
            if port_match:
                extracted_ports.add(int(port_match.group(1)))

    if violations:
        print("❌ ANTI-PREVIEW GUARD FAILED: Static preview / local file navigation detected:")
        for v in violations:
            print(f"   - {v}")
        return False, None

    print(f"✅ Anti-Preview Guard 1 passed: All test scripts target live http origins. Detected ports: {list(extracted_ports) or 'default'}")
    return True, list(extracted_ports)

def check_dev_server_liveness(port):
    print(f"🔍 [Anti-Preview Guard 2/5] Probing Dev Server liveness on port {port}...")
    url = f"http://127.0.0.1:{port}"
    max_retries = 10
    backoff = 1.5

    for attempt in range(1, max_retries + 1):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "Antigravity-Liveness-Probe"})
            with urllib.request.urlopen(req, timeout=3) as resp:
                if resp.status in (200, 302, 304, 307):
                    # EC-QA-009: Decoy Static Server Guard
                    server_header = resp.headers.get("Server", "").lower()
                    if any(s in server_header for s in ["simplehttp", "python", "serve"]):
                        print(f"❌ DECOY SERVER DETECTED: Port {port} is running a static file server ({server_header}), not the Next.js/React development server!")
                        return False

                    body_snippet = resp.read(16384).decode('utf-8', errors='ignore')
                    hydration_markers = ['id="__next"', 'id="_next"', '__NEXT_DATA__', 'vite/client', '@react-refresh', '__next_f', '_next/static']
                    
                    found_markers = [m for m in hydration_markers if m in body_snippet]
                    # Must contain at least a robust marker, an empty div is not enough
                    if not found_markers or (len(found_markers) == 1 and found_markers[0] in ['id="__next"', 'id="_next"'] and '__NEXT_DATA__' not in body_snippet and 'vite/client' not in body_snippet and '__next_f' not in body_snippet):
                        loc = resp.headers.get("Location", "").lower()
                        if not any(m in loc for m in ['login', 'auth', 'dashboard', 'workspace', 'home']):
                            print(f"❌ DECOY SERVER DETECTED: Server on {url} lacks deep React/Next.js/Vite bundle tags (e.g., __NEXT_DATA__, vite/client, __next_f).")
                            return False

                    print(f"✅ Dev Server is actively responding on {url} (HTTP {resp.status}) with verified framework markers.")
                    return True
        except (urllib.error.HTTPError, urllib.error.URLError, ConnectionRefusedError, TimeoutError) as e:
            if attempt < max_retries:
                time.sleep(backoff)
            else:
                print(f"❌ Dev Server liveness check failed on {url}: {e}")
                return False
    return False

def check_har_traffic(evidence_dir):
    print("🔍 [Anti-Preview Guard 3/5] Auditing HAR network traffic logs...")
    har_files = glob.glob(os.path.join(evidence_dir, "*.har"))
    if not har_files:
        print("⚠️ Warning: No HAR file found in evidence dir. Skipping network traffic deep-check.")
        return True

    dynamic_assets_found = 0
    localhost_traffic_found = 0

    from urllib.parse import urlparse

    for hf in har_files:
        try:
            with open(hf, "r", encoding="utf-8", errors="ignore") as f:
                data = json.load(f)
            entries = data.get("log", {}).get("entries", [])
            for entry in entries:
                url = entry.get("request", {}).get("url", "")
                parsed = urlparse(url)
                if parsed.hostname in ("localhost", "127.0.0.1", "0.0.0.0", "::1"):
                    localhost_traffic_found += 1
                if any(ext in url for ext in [".js", ".css", "_next", "/api/", "chunks", "node_modules"]):
                    dynamic_assets_found += 1
        except Exception as e:
            print(f"⚠️ Could not parse HAR {hf}: {e}")

    if localhost_traffic_found == 0 and len(har_files) > 0:
        print("❌ HAR AUDIT FAILED: Zero network requests were made to localhost.")
        return False

    print(f"✅ HAR Audit passed: {localhost_traffic_found} localhost requests, {dynamic_assets_found} dynamic asset requests.")
    return True

def check_dom_hydration(evidence_dir):
    print("🔍 [Anti-Preview Guard 4/5] Auditing captured DOM snapshots for framework hydration...")
    dom_files = glob.glob(os.path.join(evidence_dir, "*dom*.html")) + glob.glob(os.path.join(evidence_dir, "*snapshot*.html"))
    if not dom_files:
        print("⚠️ Warning: No DOM HTML snapshot found in evidence dir. Skipping hydration check.")
        return True

    framework_markers = [
        r'id=["\']__next["\']',
        r'data-reactroot',
        r'__NEXT_DATA__',
        r'_next\/static',
        r'class=["\'][^"\']*tailwind',
        r'<script[^>]*src=["\'][^"\']*\.js',
    ]

    for df in dom_files:
        with open(df, "r", encoding="utf-8", errors="ignore") as f:
            content = f.read()
        
        # Check if it looks like raw static preview.html
        if "<title>Preview" in content and "stitch" in content.lower() and not any(re.search(p, content) for p in framework_markers):
            print(f"❌ DOM AUDIT FAILED: Snapshot '{os.path.basename(df)}' is an unhydrated static preview mockup!")
            return False

        # Reject Next.js landing stub ("Go to AI Agents Hub") disguised as Tenant Portal
        if "Go to AI Agents Hub" in content and "Welcome to AI Workspace" in content:
            print(f"❌ DOM AUDIT FAILED: Snapshot '{os.path.basename(df)}' captured the Next.js stub landing page ('Go to AI Agents Hub') instead of the Vite Tenant Portal!")
            return False

    print("✅ DOM Hydration passed: Snapshots contain valid application DOM structures.")
    return True

def check_anti_copy(story_dir, evidence_dir):
    print("🔍 [Anti-Preview Guard 5/5] Checking for fraudulent duplicate screenshot copies...")
    if not story_dir or not os.path.exists(story_dir):
        return True

    # Find potential design preview mockups
    mockup_candidates = (
        glob.glob(os.path.join(story_dir, "*mockup*.*")) +
        glob.glob(os.path.join(story_dir, "*preview*.*")) +
        glob.glob(os.path.join(story_dir, "stitch*.png"))
    )

    if not mockup_candidates:
        print("ℹ️ No static preview files found in story root for direct copy comparison.")
        return True

    mockup_hashes = {}
    for m in mockup_candidates:
        if os.path.isfile(m):
            with open(m, "rb") as f:
                mockup_hashes[hashlib.sha256(f.read()).hexdigest()] = os.path.basename(m)

    evidence_screenshots = (
        glob.glob(os.path.join(evidence_dir, "*.png")) +
        glob.glob(os.path.join(evidence_dir, "*.jpg"))
    )

    for es in evidence_screenshots:
        with open(es, "rb") as f:
            es_hash = hashlib.sha256(f.read()).hexdigest()
        if es_hash in mockup_hashes:
            print(f"❌ FRAUDULENT EVIDENCE DETECTED: Evidence screenshot '{os.path.basename(es)}' is a byte-identical copy of design mockup '{mockup_hashes[es_hash]}'!")
            return False

    print("✅ Anti-Copy Check passed: No direct mockup copies detected.")
    return True

def main():
    args = parse_args()
    print(f"🛡️ Running Live Browser QA Anti-Cheat Validator for Epic {args.epic_id}, Story {args.story_id}...")

    story_dir = resolve_story_dir(args.epic_id, args.story_id, args.story_dir)
    if not story_dir:
        print(f"❌ Error: Story directory not found for Epic {args.epic_id}, Story {args.story_id}")
        sys.exit(1)

    evidence_dir = os.path.join(story_dir, "qa", "evidence")
    if not os.path.exists(evidence_dir):
        evidence_dir = f"_iwish-output/qa-evidence/Epic-{args.epic_id}/Story-{args.story_id}"

    test_files = resolve_test_files(args.epic_id, args.story_id, story_dir)
    passed_origins, ports = check_script_origins(test_files)
    if not passed_origins:
        sys.exit(1)

    if args.check_liveness:
        target_port = args.port or (ports[0] if ports else 3000)
        if not check_dev_server_liveness(target_port):
            sys.exit(1)

    if os.path.exists(evidence_dir):
        if not check_har_traffic(evidence_dir):
            sys.exit(1)
        if not check_dom_hydration(evidence_dir):
            sys.exit(1)
        if not check_anti_copy(story_dir, evidence_dir):
            sys.exit(1)
    else:
        print(f"⚠️ Evidence directory {evidence_dir} does not exist yet. Origin checks cleared.")

    print("\n✅ LIVE BROWSER QA VALIDATION PASSED: All anti-preview cheat checks cleared.")
    sys.exit(0)

if __name__ == "__main__":
    main()
