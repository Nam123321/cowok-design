import * as fs from 'fs';
import * as path from 'path';
import glob from 'glob';

const globSync = glob.sync;

export interface UIDetectionResult {
  has_ui: boolean;
  reasons: string[];
  story_id?: string;
  story_dir?: string;
  ui_spec_exists: boolean;
  ui_tags: string[];
  ui_keywords_found: string[];
}

export function detectUIStory(options: { storyDir?: string; epicId?: string; storyId?: string }): UIDetectionResult {
  let resolvedDir = options.storyDir;

  if (!resolvedDir && options.storyId) {
    const epicId = options.epicId || '*';
    const patterns = [
      `_iwish-output/3. Development/1. Epic & Story/*/Epic-${epicId}/Story-${options.storyId}`,
      `_iwish-output/3. Development/1. Epic & Story/*/Epic-${epicId}/Story-${options.storyId}.*`,
      `_iwish-output/stories/story-${options.storyId}`,
    ];
    for (const pat of patterns) {
      const matches = globSync(pat);
      if (matches.length > 0) {
        resolvedDir = matches[0];
        break;
      }
    }
  }

  const reasons: string[] = [];
  const uiTags: string[] = [];
  const uiKeywordsFound: string[] = [];
  let uiSpecExists = false;

  if (resolvedDir && fs.existsSync(resolvedDir)) {
    // 1. Check for physical ui-spec.md or equivalent
    const uiSpecPath = path.join(resolvedDir, 'ui-spec.md');
    const legacyUiSpecPath = path.join(resolvedDir, `ui-spec-story-${options.storyId}.md`);
    if (fs.existsSync(uiSpecPath) || fs.existsSync(legacyUiSpecPath)) {
      uiSpecExists = true;
      reasons.push('Physical UI specification file (ui-spec.md) exists in story directory');
    }

    // 2. Inspect story.md for UI tags and AC keywords
    const storyFile = path.join(resolvedDir, 'story.md');
    const legacyStoryFile = path.join(resolvedDir, `story-${options.storyId}.md`);
    const targetStoryFile = fs.existsSync(storyFile) ? storyFile : (fs.existsSync(legacyStoryFile) ? legacyStoryFile : null);

    if (targetStoryFile) {
      const content = fs.readFileSync(targetStoryFile, 'utf-8');

      // Check tags in frontmatter
      const tagsMatch = content.match(/tags:\s*\[(.*?)\]/s) || content.match(/tags:\n((?:\s*-\s*.*\n?)+)/);
      if (tagsMatch) {
        const rawTags = tagsMatch[1];
        const candidateTags = ['ui', 'frontend', 'ux', 'web', 'app', 'portal', 'dashboard', 'interface'];
        for (const tag of candidateTags) {
          if (new RegExp(`\\b${tag}\\b`, 'i').test(rawTags)) {
            uiTags.push(tag);
          }
        }
        if (uiTags.length > 0) {
          reasons.push(`Story frontmatter tags indicate UI: [${uiTags.join(', ')}]`);
        }
      }

      // Check for UI-related keywords in Acceptance Criteria
      const acSectionMatch = content.match(/##\s*(?:\d+\.\s*)?Acceptance Criteria([\s\S]*?)(?:##|$)/i);
      if (acSectionMatch) {
        const acContent = acSectionMatch[1];
        const uiKeywords = [
          'button', 'modal', 'dashboard', 'form', 'table', 'view', 'page',
          'screen', 'input', 'dropdown', 'dialog', 'navbar', 'sidebar',
          'card', 'badge', 'tab', 'toast', 'tooltip', 'layout', 'css'
        ];
        for (const kw of uiKeywords) {
          if (new RegExp(`\\b${kw}\\b`, 'i').test(acContent)) {
            uiKeywordsFound.push(kw);
          }
        }
        if (uiKeywordsFound.length >= 2) {
          reasons.push(`Acceptance Criteria contain UI component keywords: [${uiKeywordsFound.slice(0, 5).join(', ')}]`);
        }
      }
    }
  }

  const has_ui = uiSpecExists || uiTags.length > 0 || uiKeywordsFound.length >= 2;

  return {
    has_ui,
    reasons,
    story_id: options.storyId,
    story_dir: resolvedDir,
    ui_spec_exists: uiSpecExists,
    ui_tags: uiTags,
    ui_keywords_found: uiKeywordsFound
  };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const args = process.argv.slice(2);
  let storyDir: string | undefined;
  let epicId: string | undefined;
  let storyId: string | undefined;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--story-dir' && args[i + 1]) {
      storyDir = args[i + 1];
      i++;
    } else if (args[i] === '--epic-id' && args[i + 1]) {
      epicId = args[i + 1];
      i++;
    } else if (args[i] === '--story-id' && args[i + 1]) {
      storyId = args[i + 1];
      i++;
    } else if (!storyId && !args[i].startsWith('--')) {
      storyId = args[i];
    }
  }

  const result = detectUIStory({ storyDir, epicId, storyId });
  console.log(JSON.stringify(result, null, 2));
  process.exit(0);
}
