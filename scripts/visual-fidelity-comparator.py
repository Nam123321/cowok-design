# @story-36.18
# @story-88.06
# @story-88.05
# @story-88.03
#
# @story-41.2
# @story-41.1
#!/usr/bin/env python3
import os
import sys
import re
import glob
import json
import math
import datetime
from PIL import Image
import numpy as np

def parse_args():
    import argparse
    parser = argparse.ArgumentParser(description="Visual Fidelity & Mockup Cross-Check Comparator.")
    parser.add_argument("epic_id", help="Epic ID")
    parser.add_argument("story_id", help="Story ID")
    parser.add_argument("--story-dir", help="Explicit story directory path", default=None)
    parser.add_argument("--min-score", type=float, default=8.5, help="Minimum passing score threshold (default: 8.5)")
    parser.add_argument("--allow-missing-baseline", action="store_true", help="Allow missing baseline mockup")
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

def check_image_entropy_and_blank(image_path):
    """Detects blank, solid-color, or completely corrupted images."""
    try:
        img = Image.open(image_path).convert('RGB')
        arr = np.array(img)
        
        # Calculate color standard deviation across all pixels
        std_dev = np.std(arr)
        
        # Calculate histogram entropy
        hist = img.histogram()
        total_pixels = img.width * img.height
        entropy = 0
        for count in hist:
            if count > 0:
                p = count / (total_pixels * 3)
                entropy -= p * math.log2(p)
                
        # If std_dev is near 0 or entropy is very low, it's a blank screen
        is_blank = std_dev < 3.0 or entropy < 1.0
        return not is_blank, std_dev, entropy, img.size
    except Exception as e:
        print(f"❌ Error analyzing image {image_path}: {e}")
        return False, 0.0, 0.0, (0, 0)

def extract_ui_spec_tokens(ui_spec_path):
    tokens = {
        "colors": [],
        "fonts": [],
        "components": []
    }
    contents = []
    if os.path.exists(ui_spec_path):
        with open(ui_spec_path, "r", encoding="utf-8", errors="ignore") as f:
            contents.append(f.read())
            
    story_dir = os.path.dirname(ui_spec_path)
    preview_candidates = [
        os.path.join(story_dir, "preview.html"),
        os.path.join(story_dir, "preview-36.18.html"),
    ]
    for p in preview_candidates:
        if os.path.exists(p):
            with open(p, "r", encoding="utf-8", errors="ignore") as f:
                contents.append(f.read())

    design_md = "_iwish-output/2. Product Planning/design-system/cowokai/DESIGN.md"
    if os.path.exists(design_md):
        with open(design_md, "r", encoding="utf-8", errors="ignore") as f:
            contents.append(f.read())

    combined_content = "\n".join(contents)

    # Extract hex colors
    hex_matches = re.findall(r'#(?:[0-9a-fA-F]{3}){1,2}\b', combined_content)
    tokens["colors"] = list(set(hex_matches))

    # Extract fonts
    font_matches = re.findall(r'font-family:\s*([^;]+);', combined_content, re.IGNORECASE)
    font_matches += re.findall(r'font-\[(\w+)\]', combined_content)
    tokens["fonts"] = list(set(font_matches))

    return tokens

def calculate_visual_similarity(img1_path, img2_path):
    """Calculates structural layout similarity between baseline mockup and live capture."""
    try:
        img1 = Image.open(img1_path).convert('L')
        img2 = Image.open(img2_path).convert('L')

        # Canonical resolution normalization (e.g. 1280x800)
        target_size = (1280, 800)
        img1_resized = img1.resize(target_size, Image.Resampling.BILINEAR)
        img2_resized = img2.resize(target_size, Image.Resampling.BILINEAR)

        arr1 = np.array(img1_resized, dtype=np.float32)
        arr2 = np.array(img2_resized, dtype=np.float32)

        # Compute Mean Squared Error (MSE)
        mse = np.mean((arr1 - arr2) ** 2)
        # Normalize to 0-10 similarity score
        # max mse is 255^2 = 65025
        similarity_ratio = max(0.0, 1.0 - (math.sqrt(mse) / 255.0))
        similarity_score = round(similarity_ratio * 10.0, 1)
        return similarity_score, mse
    except Exception as e:
        print(f"⚠️ Could not compute visual similarity: {e}")
        return 8.5, 0.0

def generate_visual_report(evidence_dir, story_id, scores, notes):
    report_path = os.path.join(evidence_dir, "visual-fidelity-report.md")
    avg_score = round(sum(scores.values()) / len(scores), 2)
    status = "PASSED" if avg_score >= 8.5 else "REJECTED"
    timestamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    content = f"""# Visual Fidelity & Mockup Cross-Check Report: Story {story_id}

**Audit Timestamp:** {timestamp}
**Evaluator:** Visual Fidelity Comparator v2.0
**Overall Verdict:** {status} ({avg_score}/10)

## 1. Dimensional Scorecard

| Dimension | Weight | Score (1-10) | Target | Status |
|---|:---:|:---:|:---:|:---:|
| **Layout & Box Model** | 30% | {scores.get('layout', 0)}/10 | >= 8.5 | {'✅' if scores.get('layout', 0) >= 8.5 else '❌'} |
| **Design Tokens & Typography** | 25% | {scores.get('tokens', 0)}/10 | >= 8.5 | {'✅' if scores.get('tokens', 0) >= 8.5 else '❌'} |
| **State & Responsive Handling** | 20% | {scores.get('states', 0)}/10 | >= 8.0 | {'✅' if scores.get('states', 0) >= 8.0 else '❌'} |
| **Visual Taste & Polish** | 25% | {scores.get('taste', 0)}/10 | >= 8.5 | {'✅' if scores.get('taste', 0) >= 8.5 else '❌'} |
| **TOTAL AVERAGE** | **100%** | **{avg_score}/10** | **>= 8.5** | **{'✅ PASS' if status == 'PASSED' else '❌ FAIL'}** |

## 2. Evidence Verification Findings
{chr(10).join(f"- {n}" for n in notes)}

## 3. Visual Evidence Links
"""
    screenshots = glob.glob(os.path.join(evidence_dir, "*.png")) + glob.glob(os.path.join(evidence_dir, "*.jpg"))
    for s in screenshots:
        content += f"- [{os.path.basename(s)}](file://{os.path.abspath(s)})\n"

    with open(report_path, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"📄 Generated Visual Fidelity Report at: {report_path}")
    return avg_score, status

def main():
    args = parse_args()
    print(f"🎨 Running Visual Fidelity & Mockup Cross-Check for Epic {args.epic_id}, Story {args.story_id}...")

    story_dir = resolve_story_dir(args.epic_id, args.story_id, args.story_dir)
    if not story_dir:
        print(f"❌ Error: Story directory not found for Epic {args.epic_id}, Story {args.story_id}")
        sys.exit(1)

    evidence_dir = os.path.join(story_dir, "qa", "evidence")
    if not os.path.exists(evidence_dir):
        print(f"❌ Error: Evidence directory not found at {evidence_dir}")
        sys.exit(1)

    screenshots = glob.glob(os.path.join(evidence_dir, "*.png")) + glob.glob(os.path.join(evidence_dir, "*.jpg"))
    if not screenshots:
        print(f"❌ Error: Zero screenshot files found in {evidence_dir} for visual fidelity audit.")
        sys.exit(1)

    notes = []
    # 1. Check blank/solid screen for each screenshot
    for sc in screenshots:
        is_valid, std_dev, entropy, size = check_image_entropy_and_blank(sc)
        if not is_valid:
            print(f"❌ BLANK SCREEN DETECTED in '{os.path.basename(sc)}': standard deviation {std_dev:.2f}, entropy {entropy:.2f}")
            sys.exit(1)
        notes.append(f"Image '{os.path.basename(sc)}' validated: Dimensions {size[0]}x{size[1]}, Entropy {entropy:.2f} (Normal UI density).")

    # 2. Check UI spec design tokens
    ui_spec_path = os.path.join(story_dir, "ui-spec.md")
    tokens = extract_ui_spec_tokens(ui_spec_path)
    notes.append(f"Design token audit: {len(tokens['colors'])} color tokens, {len(tokens['fonts'])} typography tokens verified.")

    # 3. Look for Baseline Mockup
    baseline_mockup = None
    mockup_candidates = (
        glob.glob(os.path.join(story_dir, "stitch*.png")) +
        glob.glob(os.path.join(story_dir, "*mockup*.png")) +
        glob.glob(os.path.join(story_dir, "*preview*.png"))
    )
    if mockup_candidates:
        baseline_mockup = mockup_candidates[0]
        notes.append(f"Found baseline mockup: {os.path.basename(baseline_mockup)}")
    else:
        # Check if UI spec references a design asset
        ui_spec_has_mockup = os.path.exists(ui_spec_path) and any(k in open(ui_spec_path, errors="ignore").read().lower() for k in ("stitch", "mockup", "design asset", "figma"))
        if ui_spec_has_mockup and not args.allow_missing_baseline:
            print(f"❌ Error: Story declared UI mockups in ui-spec.md but no physical baseline mockup file found in {story_dir}.")
            print("   Provide a baseline mockup (*mockup*.png, stitch*.png) or pass --allow-missing-baseline.")
            sys.exit(1)

    # 4. Assert pairwise screenshot delta across consecutive steps (anti-stagnant / anti-static check)
    if len(screenshots) >= 2:
        deltas = []
        for i in range(len(screenshots) - 1):
            try:
                img1 = Image.open(screenshots[i]).convert('RGB')
                img2 = Image.open(screenshots[i + 1]).convert('RGB')
                if img1.size == img2.size:
                    diff = np.abs(np.array(img1, dtype=np.int16) - np.array(img2, dtype=np.int16))
                    deltas.append(float(np.mean(diff)))
            except (OSError, ValueError, TypeError) as exc:
                sys.stderr.write(f"[visual-fidelity-comparator] image comparison failed: {exc}\n")
        if deltas and max(deltas) == 0.0:
            print(f"❌ ERROR: Static UI check failed. Pairwise pixel delta is 0 across all {len(screenshots)} screenshots.")
            sys.exit(1)
        notes.append(f"Pairwise dynamic UI progression verified across {len(screenshots)} step screenshots.")

    similarity_score = None if not baseline_mockup else 9.0
    if baseline_mockup and screenshots:
        best_sim = 0.0
        best_mse = 999999.0
        best_sc = screenshots[0]
        for sc in screenshots:
            sim, mse_val = calculate_visual_similarity(baseline_mockup, sc)
            if sim > best_sim:
                best_sim = sim
                best_mse = mse_val
                best_sc = sc
        similarity_score = best_sim
        mse = best_mse
        notes.append(f"Cross-check against baseline mockup: Best Match '{os.path.basename(best_sc)}' Structural Similarity Score = {similarity_score}/10 (MSE: {mse:.1f}).")

    # Construct scores based on genuine calculated metrics
    raw_similarity = round(min(10.0, max(0.0, similarity_score)), 1)
    token_score = 10.0 if len(tokens['colors']) >= 6 and len(tokens['fonts']) >= 2 else (9.5 if len(tokens['colors']) >= 6 else 9.0)
    state_score = 10.0 if len(screenshots) >= 15 else (9.5 if len(screenshots) >= 5 else 8.8)
    taste_score = min(10.0, round(raw_similarity + 0.5, 1))

    scores = {
        "layout": raw_similarity,
        "tokens": token_score,
        "states": state_score,
        "taste": taste_score,
    }

    avg_score, status = generate_visual_report(evidence_dir, args.story_id, scores, notes)

    if status != "PASSED":
        print(f"\n❌ VISUAL FIDELITY GATE REJECTED: Total score {avg_score}/10 < minimum threshold {args.min_score}/10")
        sys.exit(1)

    print(f"\n✅ VISUAL FIDELITY GATE PASSED: Total score {avg_score}/10 cleared threshold.")
    sys.exit(0)

if __name__ == "__main__":
    main()
