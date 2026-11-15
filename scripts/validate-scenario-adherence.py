# @story-56.1
#!/usr/bin/env python3
import os
import sys
import re
import glob
import json

def parse_args():
    import argparse
    parser = argparse.ArgumentParser(description="Validate Scenario Adherence between manual-test-guide.md and test scripts.")
    parser.add_argument("epic_id", help="Epic ID (e.g. 01, 58)")
    parser.add_argument("story_id", help="Story ID (e.g. 01.16, 58.5)")
    parser.add_argument("--story-dir", help="Explicit story directory path", default=None)
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

def find_test_guide(story_dir, story_id):
    candidates = [
        os.path.join(story_dir, "qa", f"manual-test-guide-{story_id}.md"),
        os.path.join(story_dir, "qa", "manual-test-guide.md"),
        os.path.join(story_dir, "qa", "manual-test-spec.md"),
        os.path.join(story_dir, f"manual-test-guide-{story_id}.md"),
        os.path.join(story_dir, "manual-test-guide.md"),
    ]
    for c in candidates:
        if os.path.exists(c):
            return c
    # Fallback glob
    glob_matches = glob.glob(os.path.join(story_dir, "qa", "*test-guide*.md")) + glob.glob(os.path.join(story_dir, "qa", "*test-spec*.md"))
    return glob_matches[0] if glob_matches else None

def parse_guide_test_cases(guide_path):
    with open(guide_path, "r", encoding="utf-8", errors="ignore") as f:
        content = f.read()

    test_cases = []
    # Match headers like ### TC-01.16-001: ... or ### TC01: ... or ### Test Case 1: ...
    tc_pattern = r'###\s+(TC[-_\w\d\.]+|Test Case\s*\d+)(?::\s*|\s+)(.*?)(?=\n###|\Z)'
    matches = re.finditer(tc_pattern, content, re.DOTALL | re.IGNORECASE)

    for m in matches:
        tc_id = m.group(1).strip()
        tc_body = m.group(2).strip()
        title_line = tc_body.split('\n')[0].strip()

        # Extract steps
        steps = re.findall(r'(?:^\d+\.|\bStep\s*\d+:)\s*(.+)', tc_body, re.MULTILINE)

        # Extract expected screenshot evidence filename if mentioned
        evidence_files = re.findall(r'\*\*([a-zA-Z0-9_\-\.]+\.(?:png|jpg|jpeg))\*\*', tc_body)
        evidence_files += re.findall(r'`([a-zA-Z0-9_\-\.]+\.(?:png|jpg|jpeg))`', tc_body)

        test_cases.append({
            "id": tc_id,
            "title": title_line,
            "steps": steps,
            "expected_evidence": list(set(evidence_files))
        })
    return test_cases

def resolve_test_files(epic_id, story_id, story_dir):
    test_files = set()
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

def audit_scenario_coverage(test_cases, test_files):
    if not test_cases:
        print("⚠️ Warning: No explicit test cases (TC-*) found in test guide.")
        return True, []

    if not test_files:
        print("❌ Error: No test files found to verify scenario coverage.")
        return False, [f"Missing implementation for all {len(test_cases)} test cases"]

    all_test_content = ""
    for tf in test_files:
        with open(tf, "r", encoding="utf-8", errors="ignore") as f:
            all_test_content += f"\n--- {tf} ---\n" + f.read()

    missing_cases = []
    for tc in test_cases:
        # Match TC ID directly or normalized form
        clean_id = tc["id"].replace("-", "[-_ ]?").replace(".", r"\.")
        pattern = rf'(?:test\s*\(\s*[\'"`].*?{clean_id}|describe\s*\(\s*[\'"`].*?{clean_id})'
        if not re.search(pattern, all_test_content, re.IGNORECASE):
            # Try matching keyword words from title
            keywords = [w for w in re.findall(r'\b[a-zA-Z]{4,}\b', tc["title"].lower()) if w not in {'verify', 'should', 'test', 'ensure'}]
            matched_kw = sum(1 for kw in keywords if kw in all_test_content.lower())
            if matched_kw < max(2, len(keywords) // 2):
                missing_cases.append(f"{tc['id']} ({tc['title']})")

    return len(missing_cases) == 0, missing_cases

def audit_evidence_artifacts(test_cases, evidence_dir):
    if not os.path.exists(evidence_dir):
        return False, ["Evidence directory does not exist"]

    missing_evidence = []
    files_in_dir = os.listdir(evidence_dir)

    for tc in test_cases:
        if tc["expected_evidence"]:
            for ef in tc["expected_evidence"]:
                found = False
                for f in files_in_dir:
                    if f.lower() == ef.lower():
                        full_path = os.path.join(evidence_dir, f)
                        if os.path.getsize(full_path) >= 1024:
                            found = True
                            break
                if not found:
                    missing_evidence.append(f"{tc['id']}: missing expected artifact '{ef}' (or size < 1024B)")
        else:
            # Check for general screenshot with TC id in filename
            clean_id = tc["id"].lower().replace("test case", "tc").replace(" ", "").replace("_", "-")
            found = any(clean_id in f.lower() and os.path.getsize(os.path.join(evidence_dir, f)) >= 1024 for f in files_in_dir)
            if not found:
                missing_evidence.append(f"{tc['id']}: no matching screenshot evidence found in {evidence_dir}")

    return len(missing_evidence) == 0, missing_evidence

def main():
    args = parse_args()
    print(f"📋 Running Scenario Adherence Validator for Epic {args.epic_id}, Story {args.story_id}...")

    story_dir = resolve_story_dir(args.epic_id, args.story_id, args.story_dir)
    if not story_dir:
        print(f"❌ Error: Story directory not found for Epic {args.epic_id}, Story {args.story_id}")
        sys.exit(1)

    guide_path = find_test_guide(story_dir, args.story_id)
    if not guide_path:
        print(f"❌ Error: No manual test guide found in {story_dir}/qa/")
        sys.exit(1)

    print(f"📄 Found Test Guide: {guide_path}")
    test_cases = parse_guide_test_cases(guide_path)
    print(f"🔢 Parsed {len(test_cases)} Test Case(s) from guide:")
    for tc in test_cases:
        print(f"   - {tc['id']}: {tc['title']} ({len(tc['steps'])} steps, {len(tc['expected_evidence'])} expected artifacts)")

    test_files = resolve_test_files(args.epic_id, args.story_id, story_dir)
    coverage_ok, missing_cases = audit_scenario_coverage(test_cases, test_files)
    if not coverage_ok:
        print("\n❌ SCENARIO ADHERENCE FAILED: The test scripts do not implement all required test cases:")
        for mc in missing_cases:
            print(f"   - Missing: {mc}")
        print("   Fix: Add dedicated test() blocks for each Test Case defined in the test guide.")
        sys.exit(1)

    evidence_dir = os.path.join(story_dir, "qa", "evidence")
    evidence_ok, missing_evidence = audit_evidence_artifacts(test_cases, evidence_dir)
    if not evidence_ok:
        print("\n❌ SCENARIO EVIDENCE AUDIT FAILED:")
        for me in missing_evidence:
            print(f"   - {me}")
        sys.exit(1)

    print(f"\n✅ SCENARIO ADHERENCE PASSED: 100% test case coverage verified ({len(test_cases)}/{len(test_cases)} cases implemented with valid evidence).")
    sys.exit(0)

if __name__ == "__main__":
    main()
