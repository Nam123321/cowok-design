import os, sys
# --- [Watchmen Core Injection] ---
_script_dir = os.path.dirname(os.path.abspath(__file__))
_agent_dir = os.path.abspath(os.path.join(_script_dir, ".."))
if _agent_dir not in sys.path:
    sys.path.insert(0, _agent_dir)
try:
    import watchmen_core
    watchmen_core.verify_execution(__file__)
except ImportError:
    pass
# ---------------------------------

import json
import argparse
import datetime
import hashlib
from pathlib import Path

def emit_evidence(story_dir_path, engine_name, status="completed", details=None):
    story_dir = Path(story_dir_path).resolve()
    evidence_file = story_dir / "code-stage-evidence.json"
    
    timestamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
    
    payload = {
        "stage": "3B",
        "phase": "code",
        "status": status,
        "executed_by": engine_name,
        "timestamp": timestamp,
        "story_dir": str(story_dir),
        "details": details or {}
    }
    
    # Calculate SHA-256 canonical hash of payload content for cryptographic traceability
    canonical_bytes = json.dumps(payload, sort_keys=True, ensure_ascii=False).encode("utf-8")
    payload_hash = hashlib.sha256(canonical_bytes).hexdigest()
    
    evidence_data = {
        **payload,
        "signature": {
            "algo": "sha256",
            "hash": payload_hash
        }
    }
    
    with open(evidence_file, "w", encoding="utf-8") as f:
        json.dump(evidence_data, f, indent=2, ensure_ascii=False)
        
    print(f"✅ Emitted Code Stage Evidence: {evidence_file}")
    print(f"   Executed By: {engine_name}")
    print(f"   Status: {status}")
    print(f"   Signature: {payload_hash}")
    return evidence_file

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Emit structured Stage 3B Code Evidence")
    parser.add_argument("--story-dir", required=True, help="Path to story directory")
    parser.add_argument("--engine", required=True, help="Agent/Engine identifier")
    parser.add_argument("--status", default="completed", help="Completion status")
    parser.add_argument("--details", help="Optional JSON string of extra details")
    args = parser.parse_args()
    
    extra = None
    if args.details:
        try:
            extra = json.loads(args.details)
        except Exception as e:
            extra = {"raw_details": args.details, "parse_error": str(e)}
            
    emit_evidence(args.story_dir, args.engine, args.status, extra)
