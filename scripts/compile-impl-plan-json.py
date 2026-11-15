#!/usr/bin/env python3
import os, sys
import re
import json
import argparse
from pathlib import Path
import jsonschema

_script_dir = Path(__file__).resolve().parent
_agent_scripts_dir = (_script_dir.parent / ".agent" / "scripts").resolve()
if str(_agent_scripts_dir) not in sys.path:
    sys.path.insert(0, str(_agent_scripts_dir))

from runtime_paths import (
    resolve_project_root,
    infer_story_id_and_epic_id,
    extract_markdown_file_manifest,
)

def parse_frontmatter(content):
    match = re.match(r"^(?:<[^>]+>\s*\n)?---\s*\n(.*?)\n---\s*\n", content, re.DOTALL)
    if not match:
        return {}
    meta = {}
    for line in match.group(1).splitlines():
        if ":" in line:
            k, v = line.split(":", 1)
            meta[k.strip()] = v.strip().strip("'\"")
    return meta

def detect_default_test_and_typecheck(repo_root: Path):
    """EC-HARNESS-P9-001: Dynamically detect test and typecheck commands from repository manifests."""
    test_cmd = "npm test"
    typecheck_cmd = "npx tsc --noEmit"
    if (repo_root / "pnpm-lock.yaml").exists():
        test_cmd = "pnpm test"
        typecheck_cmd = "pnpm tsc --noEmit"
    elif (repo_root / "pyproject.toml").exists() or (repo_root / "pytest.ini").exists():
        test_cmd = f"{sys.executable} -m pytest"
        typecheck_cmd = f"{sys.executable} -m py_compile"
    return test_cmd, typecheck_cmd

def extract_file_manifest(content, repo_root: Path = None):
    root = resolve_project_root(explicit_root=repo_root)
    target_files = extract_markdown_file_manifest(content, repo_root=root)

    has_source = any(not ("test" in p or "spec" in p) for p in (target_files["create"] + target_files["modify"]))
    if not has_source or (not target_files["create"] and not target_files["modify"] and not target_files["delete"]):
        # Expand brace patterns like src/plugins/{a,b}/file.ts
        expanded_content = content
        for m in re.finditer(r"([A-Za-z0-9_\-]+/[A-Za-z0-9_\-/]*)\{([A-Za-z0-9_\-,]+)\}(/[A-Za-z0-9_\-/\.]*)?", content):
            prefix, items_str, suffix = m.group(1), m.group(2), m.group(3) or "/"
            expanded = " ".join(f"{prefix}{it.strip()}{suffix}" for it in items_str.split(",") if it.strip())
            expanded_content += "\n" + expanded

        raw_paths = re.findall(
            r"`?([A-Za-z0-9_\-]+/[A-Za-z0-9_\-/\.\[\]]+\.[A-Za-z0-9]+)`?",
            expanded_content,
        )
        for p in raw_paths:
            clean_p = p.strip().rstrip(".,:;)")
            if not clean_p or ".." in clean_p or clean_p.endswith("*") or clean_p.startswith((".git/", "_iwish-output/")):
                continue
            if not any(clean_p.startswith(prefix) for prefix in ("src/", "packages/", "apps/", "tests/", "prisma/")):
                continue
            full_p = root / clean_p
            if not full_p.exists() or full_p.is_dir():
                continue
            if clean_p not in target_files["modify"] and clean_p not in target_files["create"]:
                target_files["modify"].append(clean_p)

    for p in list(target_files["modify"]) + list(target_files["create"]):
        if p.endswith(".service.ts"):
            types_p = p.replace(".service.ts", ".types.ts")
            if (root / types_p).is_file() and types_p not in target_files["create"] and types_p not in target_files["modify"]:
                target_files["modify"].append(types_p)

    return target_files

def extract_tasks(task_content, plan_content, target_files=None, default_test_cmd="pnpm test"):
    tasks = []
    content = task_content if task_content else plan_content
    task_lines = re.findall(r"-\s*\[[ xX]\]\s*(?:\*\*|`)?(?:TASK-([0-9\.\-_]+)|Task\s+([0-9\.\-_]+)|([0-9\.\-_]+))(?:\:)?(?:\*\*|`)?(?:\s*[:\-])?\s*(.+)", content, re.IGNORECASE)
    idx = 1
    declared_targets = []
    if target_files:
        declared_targets = target_files.get("create", []) + target_files.get("modify", []) + target_files.get("delete", [])

    for match in task_lines:
        task_id = match[0] or match[1] or match[2] or f"{idx}"
        if not task_id.upper().startswith("TASK-"):
            task_id = f"TASK-{task_id.lstrip('-')}"
        desc = match[3].strip()
        
        ac_matches = re.findall(r"AC-?[0-9]+", desc, re.IGNORECASE)
        ac_list = [ac.upper() for ac in ac_matches] if ac_matches else ["AC-1"]
        
        file_match = re.search(r"`?([A-Za-z0-9_\-]+/[A-Za-z0-9_\-/\.\[\]]+\.[A-Za-z0-9]+)`?", desc)
        target_path = file_match.group(1) if file_match else ""
        
        if ".." in target_path or target_path.startswith("/") or target_path.startswith(".git/") or target_path.startswith("_iwish-output/"):
            target_path = ""
            
        checker_ids = ["vitest"] if (target_path.endswith((".ts", ".tsx")) or any(t.endswith((".ts", ".tsx")) for t in declared_targets)) else ["pytest"] if target_path.endswith(".py") else ["vitest"]
        task_targets = declared_targets if declared_targets else ([target_path] if target_path else [])
        if not task_targets:
            print(f"❌ [FAIL-CLOSED] Error: Task {task_id} has no target files and File Manifest is empty. Refusing to inject phantom fallback files.", file=sys.stderr)
            sys.exit(1)
            
        tasks.append({
            "task_id": task_id.upper(),
            "name": desc,
            "target": target_path or task_targets[0],
            "target_files": task_targets,
            "ac_covered": ac_list,
            "test_command": default_test_cmd,
            "checker_ids": checker_ids
        })
        idx += 1
        
    if not tasks:
        if "[MODIFY]" in content or "[NEW]" in content or "[DELETE]" in content or "TASK-" in content.upper():
            print("❌ Error: Task markers found in markdown, but no tasks were successfully extracted. Check regex or task format.", file=sys.stderr)
            sys.exit(1)
        if not declared_targets:
            print("❌ [FAIL-CLOSED] Error: No target files declared in impl-plan.md File Manifest. Refusing to inject phantom 'src/index.ts'.", file=sys.stderr)
            sys.exit(1)
        tasks.append({
            "task_id": "TASK-1",
            "name": "Implement core story logic and automated tests",
            "target": declared_targets[0],
            "target_files": declared_targets,
            "ac_covered": ["AC-1"],
            "test_command": default_test_cmd,
            "checker_ids": ["vitest"]
        })
    return tasks

def extract_testing_commands(content, repo_root: Path):
    test_cmd, typecheck_cmd = detect_default_test_and_typecheck(repo_root)
    
    cmd_match = re.search(r"```(?:bash|sh)\s*\n(.*?(?:pnpm|npm|npx|pytest|vitest)\s+[^\n]+)\n```", content, re.IGNORECASE)
    if cmd_match:
        test_cmd = cmd_match.group(1).strip().splitlines()[0]
        
    return {
        "test_command": test_cmd,
        "typecheck_command": typecheck_cmd,
        "required_coverage_percent": 80.0
    }

def resolve_execution_engine_and_shared_types(story_dir: Path, target_files: dict, frontmatter: dict) -> tuple[str, str]:
    """Dynamically resolves execution_engine and shared_types_file without static literals."""
    engine = frontmatter.get("execution_engine") or ""
    if not engine:
        sel_file = story_dir / "code-engine-selection.json"
        if sel_file.is_file():
            try:
                sel_data = json.loads(sel_file.read_text(encoding="utf-8"))
                engine = str(sel_data.get("selected_engine") or sel_data.get("engine") or "").lstrip("/")
            except (OSError, json.JSONDecodeError, ValueError, TypeError, AttributeError) as exc:
                sys.stderr.write(f"[compile-impl-plan-json] invalid engine selection in {sel_file}: {exc}\n")
    if not engine:
        engine = "pi-code-agent"

    all_targets = (target_files.get("create") or []) + (target_files.get("modify") or [])
    shared_types = ""
    for p in all_targets:
        if "types" in p.lower() or "validations" in p.lower() or "schema" in p.lower():
            shared_types = p
            break
    if not shared_types and all_targets:
        shared_types = all_targets[0]
    return engine, shared_types

def compile_plan(story_dir_path, explicit_root=None):
    story_dir = Path(story_dir_path).resolve()
    repo_root = resolve_project_root(explicit_root=explicit_root, target_path=story_dir)
    plan_file = story_dir / "impl-plan.md"
    task_file = story_dir / "task.md"
    
    if not plan_file.exists():
        print(f"❌ Error: {plan_file} does not exist.")
        sys.exit(1)
        
    plan_content = plan_file.read_text(encoding="utf-8")
    task_content = task_file.read_text(encoding="utf-8") if task_file.exists() else ""
    
    frontmatter = parse_frontmatter(plan_content)
    sid, eid = infer_story_id_and_epic_id(
        story_dir=story_dir,
        file_path=plan_file,
        content=plan_content,
        explicit_story_id=frontmatter.get("story_id") or frontmatter.get("id"),
    )
    story_id = f"story-{sid}" if sid else "story-0.1"
    epic_id = f"Epic-{eid}" if eid else "Epic-0"
        
    target_files = extract_file_manifest(plan_content, repo_root=repo_root)
    for category in ["create", "modify", "delete"]:
        sanitized = []
        for p in target_files[category]:
            if ".." in p or p.startswith("/") or p.startswith(".git/") or p.startswith("_iwish-output/"):
                print(f"⚠️ Warning: Sanitizing forbidden target path: {p}")
                continue
            sanitized.append(p)
        target_files[category] = sanitized
        
    if not target_files["create"] and not target_files["modify"] and not target_files["delete"]:
        print(f"❌ [FAIL-CLOSED] Error: {plan_file} yielded 0 target files in File Manifest. Refusing to inject phantom 'src/index.ts'.", file=sys.stderr)
        sys.exit(1)

    testing = extract_testing_commands(plan_content, repo_root)
    tasks = extract_tasks(task_content, plan_content, target_files, default_test_cmd=testing["test_command"])
    exec_engine, shared_types = resolve_execution_engine_and_shared_types(story_dir, target_files, frontmatter)

    contract_data = {
        "story_id": story_id,
        "epic_id": epic_id,
        "execution_engine": exec_engine,
        "target_files": target_files,
        "contracts": {
            "shared_types_file": shared_types,
            "interfaces": []
        },
        "tasks": tasks,
        "testing": testing
    }
    
    schema_path = (repo_root / ".agent" / "schemas" / "impl-plan.schema.json").resolve()
    if not schema_path.exists():
        schema_path = (Path(__file__).resolve().parent.parent / ".agent" / "schemas" / "impl-plan.schema.json").resolve()
    if not schema_path.exists():
        print(f"❌ Critical Error: Schema not found at {schema_path}")
        sys.exit(1)
        
    with open(schema_path, "r", encoding="utf-8") as sf:
        schema = json.load(sf)
    try:
        jsonschema.validate(instance=contract_data, schema=schema)
        print("🛡️ Schema Validation: PASS (Conforms to impl-plan.schema.json)")
    except jsonschema.ValidationError as ve:
        print(f"❌ Schema Validation Error: {ve.message}")
        sys.exit(1)
            
    out_file = story_dir / "impl-plan.json"
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(contract_data, f, indent=2, ensure_ascii=False)
        
    print(f"✅ Successfully compiled machine contract: {out_file}")
    return out_file

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Compile impl-plan.md into impl-plan.json")
    parser.add_argument("--story-dir", required=True, help="Path to story directory")
    parser.add_argument("--root", required=False, default=None, help="Explicit repository/worktree root")
    args = parser.parse_args()
    
    compile_plan(args.story_dir, explicit_root=args.root)
