import fs from 'fs';
import path from 'path';
import crypto from 'crypto';
import { fileURLToPath } from 'url';
import ts from 'typescript';

const SELF_FILE_PATH = path.resolve(fileURLToPath(import.meta.url));

// Parse command line arguments
const args = process.argv.slice(2);
let files = [];
let storyDir = '';
let storyId = '';
let hasExplicitFiles = false;
const EXCLUDED_SCAN_DIRS = new Set([
  'node_modules', '.next', 'dist', 'build', 'target', '.venv', 'venv',
  '__pycache__', '.turbo', 'out', 'coverage', '.git', '.worktrees'
]);

for (let i = 0; i < args.length; i++) {
  if (args[i] === '--files') {
    hasExplicitFiles = true;
    i++;
    while (i < args.length && !args[i].startsWith('--')) {
      const parts = args[i].split(',');
      for (const p of parts) {
        if (p.trim()) files.push(p.trim());
      }
      i++;
    }
    i--;
  } else if (args[i] === '--story-dir' || args[i] === '--dir') {
    storyDir = args[++i];
  } else if (args[i] === '--story-id' || args[i] === '--story') {
    storyId = args[++i];
  }
}

if (!storyDir && storyId) {
  const cleanId = storyId.replace(/^Story-/, '');
  const searchBases = [
    path.join(process.cwd(), '_iwish-output', '3. Development', '1. Epic & Story'),
    path.join(process.cwd(), '_iwish-output', 'stories'),
    path.join(process.cwd(), '_iwish-output'),
  ];
  for (const base of searchBases) {
    if (!fs.existsSync(base)) continue;
    const findStoryDir = (current) => {
      try {
        const entries = fs.readdirSync(current, { withFileTypes: true });
        for (const ent of entries) {
          if (ent.isDirectory()) {
            const full = path.join(current, ent.name);
            if (ent.name === `Story-${cleanId}` || ent.name === cleanId) {
              return full;
            }
            const nested = findStoryDir(full);
            if (nested) return nested;
          }
        }
      } catch (err) {
        console.warn(`[anti-cheat-linter] failed scanning directory ${current}: ${err?.message || err}`);
      }
      return null;
    };
    const found = findStoryDir(base);
    if (found) {
      storyDir = found;
      break;
    }
  }
}

const getFilesRecursive = (dir) => {
  let results = [];
  if (!fs.existsSync(dir)) return results;
  const list = fs.readdirSync(dir);
  for (const file of list) {
    if (EXCLUDED_SCAN_DIRS.has(file)) continue;
    const filePath = path.join(dir, file);
    const stat = fs.statSync(filePath);
    if (stat && stat.isDirectory()) {
      results = results.concat(getFilesRecursive(filePath));
    } else {
      results.push(filePath);
    }
  }
  return results;
};

// Fallback to story impl-plan.json target_files or traceability.json or recursive scan of src if no files specified
let allStoryTargetFiles = [];
if (files.length === 0 && storyDir && fs.existsSync(path.join(storyDir, 'impl-plan.json'))) {
  try {
    const plan = JSON.parse(fs.readFileSync(path.join(storyDir, 'impl-plan.json'), 'utf8'));
    const tFiles = [
      ...(plan.target_files?.modify || []),
      ...(plan.target_files?.create || []),
      ...(plan.tasks?.flatMap(t => [...(t.target_files || []), ...(t.files || []), t.target].filter(Boolean)) || []),
      ...(plan.test_files || [])
    ];
    allStoryTargetFiles = [...new Set(tFiles)];
    files = allStoryTargetFiles.filter(f => /\.(ts|tsx|js|jsx)$/.test(f));
  } catch (e) {
    console.warn(`[anti-cheat-linter] failed parsing impl-plan.json in ${storyDir}: ${e?.message || e}`);
  }
}

if (files.length === 0 && storyDir && fs.existsSync(path.join(storyDir, 'traceability.json'))) {
  try {
    const trace = JSON.parse(fs.readFileSync(path.join(storyDir, 'traceability.json'), 'utf8'));
    const tFiles = [
      ...((trace.criteria || trace.traceability_matrix || []).flatMap(row => [
        ...(row.implementations || []),
        ...(row.tests || [])
      ])),
      ...(trace.files || []),
      ...(trace.code_files || []),
      ...(trace.test_files || [])
    ].map(item => typeof item === 'string' ? item : item?.path).filter(Boolean);
    allStoryTargetFiles = [...new Set([...allStoryTargetFiles, ...tFiles])];
    files = allStoryTargetFiles.filter(f => /\.(ts|tsx|js|jsx)$/.test(f));
  } catch (e) {
    console.warn(`[anti-cheat-linter] failed parsing traceability.json in ${storyDir}: ${e?.message || e}`);
  }
}

if (files.length === 0 && storyDir && fs.existsSync(path.join(storyDir, 'task-traceability.json'))) {
  try {
    const ttrace = JSON.parse(fs.readFileSync(path.join(storyDir, 'task-traceability.json'), 'utf8'));
    const tFiles = (ttrace.tasks || []).flatMap(t => [...(t.target_files || []), ...(t.files || [])]);
    allStoryTargetFiles = [...new Set([...allStoryTargetFiles, ...tFiles])];
    files = allStoryTargetFiles.filter(f => /\.(ts|tsx|js|jsx)$/.test(f));
  } catch (e) {
    console.warn(`[anti-cheat-linter] failed parsing task-traceability.json in ${storyDir}: ${e?.message || e}`);
  }
}

if (files.length === 0) {
  if (storyId) {
    if (allStoryTargetFiles.length > 0) {
      console.log(`[anti-cheat-linter] Story ${storyId} has ${allStoryTargetFiles.length} target files (0 JS/TS source files to lint). PASS.`);
      process.exit(0);
    }
    console.error(`[Anti-Cheat Fail-Closed] No target files found for story ${storyId}. Cannot proceed.`);
    process.exit(1);
  }
  const defaultScanDirs = ['src', 'apps', 'distro', 'server', 'tests', 'packages', 'scripts'];
  files = defaultScanDirs.filter(sDir => fs.existsSync(sDir)).flatMap(getFilesRecursive).filter(f =>
    /\.(ts|tsx|js|jsx)$/.test(f) && 
    !/\.test\./.test(f) && 
    !/\.spec\./.test(f) &&
    !f.includes('/__tests__/') &&
    !f.includes('/tests/') &&
    path.resolve(f) !== SELF_FILE_PATH
  );
}

// Parallel story isolation: When storyId or explicit files are passed, only scan test files belonging to that story.
const scanDirs = ['tests', 'src', 'apps', 'distro', 'server', 'packages', 'scripts'];
for (const sDir of scanDirs) {
  if (fs.existsSync(sDir)) {
    const allCandidates = getFilesRecursive(sDir).filter(f => /\.(ts|tsx|js|jsx)$/.test(f) && path.resolve(f) !== SELF_FILE_PATH);
    for (const cf of allCandidates) {
      if (!files.includes(cf)) {
        try {
          const head = fs.readFileSync(cf, 'utf8');
          if (storyId) {
            const cleanId = storyId.replace(/\./g, '-');
            const escapedStoryId = storyId.replace(/\./g, '\\.');
            const exactTagRe = new RegExp(`@story-(?:${escapedStoryId}|${cleanId})(?![0-9])`);
            const exactPathRe = new RegExp(`(?:^|[^0-9])${cleanId}(?![0-9])`);
            if (exactTagRe.test(head) || exactPathRe.test(cf)) {
              files.push(cf);
            }
          } else if (!hasExplicitFiles) {
            if (head.includes('@iwish-sealed-oracle')) {
              files.push(cf);
            }
          }
        } catch (err) {
          console.warn(`[anti-cheat-linter] failed reading candidate file ${cf}: ${err?.message || err}`);
        }
      }
    }
  }
}

const failures = [];
const findings = [];
const gateSpoofComment = /\b(?:bypass|satisfy|trick|spoof|placate|silence|evade|fool)\b[\s\S]{0,60}\b(?:audit|linter|gate|validator|check|guard|anti[-_ ]?cheat)\b|\/\/\s*Token\s+markers\b|\b(?:MockComponent\s+AnotherComponent|NoUIComponent\s+PlaceholderPage)\b/i;
const registryBadName = /(?:^|_)(?:mock|dummy|fake|stub|temp|todo|tbd|placeholder)(?:_|$)|^(?:MockModel|IntegrationDummy|DummyEntity|TestModel|SampleModel|PoC_Domain\w*)$/i;
const registryBadNote = /(?:mock|dummy|placeholder|todo|tbd|fixme|temporary)/i;
let mockCount = 0;
let allMocksApproved = true;

// ━━━ ORIGINAL CHECKS (Scoped: only run if target files are in active scan scope) ━━━
const requiredFiles = [
  'server/routes/default_skill_packs.js',
  'server/tests/integration/default_skill_packs.test.js'
];

const requiredMarkers = [
  ['server/routes/default_skill_packs.js', 'pg_advisory_xact_lock'],
  ['server/routes/default_skill_packs.js', 'findTenantAgentById'],
  ['server/routes/default_skill_packs.js', 'scanCode'],
  ['server/routes/default_skill_packs.js', 'requiredRuntimes'],
  ['server/tests/integration/default_skill_packs.test.js', 'Promise.all'],
  ['server/tests/integration/default_skill_packs.test.js', 'forged-agent-id'],
  ['server/tests/integration/default_skill_packs.test.js', 'RUNTIME'],
  ['server/tests/integration/default_skill_packs.test.js', 'QUARANTINED']
];

if (files.some(f => f.includes('default_skill_packs.js'))) {
  if (fs.existsSync('server/routes/default_skill_packs.js') && storyId === 'default_skill_packs') {
    for (const file of requiredFiles) {
      if (!fs.existsSync(file)) {
        failures.push(`Missing required file: ${file}`);
      }
    }
  }

  if (fs.existsSync('server/routes/default_skill_packs.js')) {
    for (const [file, marker] of requiredMarkers) {
      if (!fs.existsSync(file)) continue;
      const source = fs.readFileSync(file, 'utf8');
      if (!source.includes(marker)) {
        failures.push(`Missing anti-cheat marker "${marker}" in ${file}`);
      }
    }
  }
}

// ━━━ ORIGINAL LOCALIZATION GUARDIAN (Scoped: only run if target files are in active scan scope) ━━━
const targetFilesToCheck = [
  'src/features/settings/personal/PersonalInfoSettings.jsx',
  'src/features/settings/personal/RegionalSettings.jsx',
  'src/features/settings/workspace/WorkspaceRegionalSettings.jsx',
  'src/components/layout/ProfilePopover.jsx'
];

for (const file of targetFilesToCheck) {
  if (!files.some(f => path.normalize(f) === path.normalize(file))) continue;
  if (!fs.existsSync(file)) continue;
  const source = fs.readFileSync(file, 'utf8');

  // 1. Scan JSX text nodes: >Text<
  const textNodes = source.match(/>\s*([a-zA-Z][^<>{}\n]*)\s*</g) || [];
  for (const match of textNodes) {
    const text = match.slice(1, -1).trim();
    if (text && !text.includes('//') && !text.startsWith('/*')) {
      failures.push(`[Localization Linter] Hardcoded text node found in ${file}: "${text}". Wrap in {t('key')}`);
    }
  }

  // 2. Scan translatable props: label="Text", title="Text", placeholder="Text"
  const props = source.match(/\b(label|title|placeholder|description)="([^"]*[a-zA-Z][^"]*)"/g) || [];
  for (const match of props) {
    failures.push(`[Localization Linter] Hardcoded prop found in ${file}: ${match}. Use dynamic curly braces {t('key')}`);
  }
}

// ━━━ NEW ANTI-CHEAT SCANNING PATTERNS ━━━
const emptyStubPatterns = [
  /return\s*(\{\s*\}|\[\s*\]|null|undefined)\s*;?\s*(?:\/\/\s*[Mm]ock|\/\*\s*[Mm]ock\*\/)/i,
  /const\s+\w*mock\w*\s*=\s*.*=>\s*(\{\s*\}|\[\s*\])/i,
  /function\s+\w*mock\w*\s*\(.*\)\s*\{\s*return\s*(\{\s*\}|\[\s*\])?\s*\}/i,
  /return\s*['"`]mock['"`]/i
];

const authMockPattern = /['"`](?:mock-tenant|mock-user|test-tenant|mock-\w+-id)['"`]/i;
const todoPattern = /\b(TODO|FIXME|HACK|IMPLEMENT_ME)\b/;
const fakeDataPattern = /\b(mockData|dummyData|fakeResponse|MOCK_[A-Z0-9_]+|SAMPLE_DATA|SAMPLE_GRID_DATA|INITIAL_REQUESTS)\b|const\s+(?:INITIAL|SAMPLE)_[A-Z0-9_]+\s*(?::\s*[^=]+)?=\s*\[\s*\{/i;
const setTimeoutSimulate = /setTimeout\s*\(.*[Ss]imulat/;

function checkSealedOracleIntegrity(file, content) {
  const startMarker = '// @iwish-sealed-oracle-start';
  const endMarker = '// @iwish-sealed-oracle-end';

  const hasStart = content.includes(startMarker);
  const hasEnd = content.includes(endMarker);

  if (!hasStart && !hasEnd) {
    return;
  }

  if (hasStart !== hasEnd) {
    findings.push({
      file,
      line: 0,
      pattern: 'sealed_oracle_tampered',
      severity: 'critical',
      text: 'SEALED_ORACLE_TAMPERED: Mismatched sealed oracle markers (start or end marker missing).',
      mock_approved: false
    });
    allMocksApproved = false;
    return;
  }

  const startIndex = content.indexOf(startMarker);
  const endIndex = content.indexOf(endMarker);

  if (endIndex <= startIndex) {
    findings.push({
      file,
      line: 0,
      pattern: 'sealed_oracle_tampered',
      severity: 'critical',
      text: 'SEALED_ORACLE_TAMPERED: @iwish-sealed-oracle-end appears before @iwish-sealed-oracle-start.',
      mock_approved: false
    });
    allMocksApproved = false;
    return;
  }

  const shaMatch = content.match(/\/\/\s*@oracle-sha256:\s*([a-f0-9]{64})/i);
  if (!shaMatch) {
    findings.push({
      file,
      line: 0,
      pattern: 'sealed_oracle_tampered',
      severity: 'critical',
      text: 'SEALED_ORACLE_TAMPERED: Sealed oracle block present but missing valid // @oracle-sha256: header.',
      mock_approved: false
    });
    allMocksApproved = false;
    return;
  }

  const expectedSha = shaMatch[1].toLowerCase();
  const sealedContent = content.substring(startIndex + startMarker.length, endIndex);
  const normalized = sealedContent.replace(/\r\n/g, '\n').trim();
  const actualSha = crypto.createHash('sha256').update(normalized).digest('hex');

  if (actualSha !== expectedSha) {
    findings.push({
      file,
      line: 0,
      pattern: 'sealed_oracle_tampered',
      severity: 'critical',
      text: `SEALED_ORACLE_TAMPERED: Oracle block SHA mismatch (expected: ${expectedSha}, actual: ${actualSha}). Assertions in sealed block were modified.`,
      mock_approved: false
    });
    allMocksApproved = false;
  }
}

for (const file of files) {
  if (!fs.existsSync(file)) continue;
  if (path.resolve(file) === SELF_FILE_PATH) continue;

  const content = fs.readFileSync(file, 'utf8');
  if (gateSpoofComment.test(content)) findings.push({ file, line: 0, pattern: 'gate_spoof_comment', severity: 'critical', text: 'Spoofing language or synthetic token markers aimed at an audit/gate is forbidden.', mock_approved: false });
  if (file.replace(/\\/g, '/').endsWith('packages/database/prisma/domain-boundary-map.json')) {
    try {
      const registry = JSON.parse(content);
      const visit = (v, key = '') => {
        if (typeof v === 'string' && ((/(?:model|entity|name|key)/i.test(key) && registryBadName.test(v)) || (/^(notes|comment)$/i.test(key) && registryBadNote.test(v))))
          findings.push({ file, line: 0, pattern: 'registry_spoof_data', severity: 'critical', text: `Forbidden registry value in ${key}`, mock_approved: false });
        if (Array.isArray(v)) v.forEach(x => visit(x, key)); else if (v && typeof v === 'object') Object.entries(v).forEach(([k,x]) => { if ((k === 'entities' || /model/i.test(k)) && typeof x === 'string' && registryBadName.test(x)) findings.push({file,line:0,pattern:'registry_spoof_data',severity:'critical',text:`Forbidden registry identifier ${x}`,mock_approved:false}); visit(x,k); });
      }; visit(registry);
      if (Array.isArray(registry.boundedContexts)) {
        let sumModels = 0;
        const domainModelSets = new Map();
        const sharedEntities = new Set((registry.sharedKernel && Array.isArray(registry.sharedKernel.entities)) ? registry.sharedKernel.entities : []);
        for (const ctx of registry.boundedContexts) {
          const actualLen = Array.isArray(ctx.models) ? ctx.models.length : 0;
          sumModels += actualLen;
          domainModelSets.set(ctx.id, new Set(ctx.models || []));
          if (typeof ctx.modelCount === 'number' && ctx.modelCount !== actualLen) {
            findings.push({
              file,
              line: 0,
              pattern: 'registry_count_mismatch',
              severity: 'critical',
              text: `Domain boundary registry mismatch in '${ctx.id}': declared modelCount=${ctx.modelCount} !== actual models.length=${actualLen}.`,
              mock_approved: false
            });
          }
        }
        if (typeof registry.totalModelsMapped === 'number' && registry.totalModelsMapped !== sumModels && registry.totalModelsMapped !== sumModels + sharedEntities.size) {
          findings.push({
            file,
            line: 0,
            pattern: 'registry_total_count_mismatch',
            severity: 'critical',
            text: `Domain boundary registry totalModelsMapped=${registry.totalModelsMapped} does not match sum of context models (${sumModels}).`,
            mock_approved: false
          });
        }
        if (Array.isArray(registry.severedForeignKeys)) {
          for (const fk of registry.severedForeignKeys) {
            const pSet = domainModelSets.get(fk.parentDomain);
            if (!((pSet && pSet.has(fk.parentModel)) || sharedEntities.has(fk.parentModel))) {
              findings.push({
                file,
                line: 0,
                pattern: 'registry_dangling_severed_fk',
                severity: 'critical',
                text: `severedForeignKey '${fk.childModel}.${fk.field}' references parent '${fk.parentDomain}.${fk.parentModel}' which is absent from both '${fk.parentDomain}' and sharedKernel.`,
                mock_approved: false
              });
            }
          }
        }
      }
    } catch { findings.push({file,line:0,pattern:'registry_invalid_json',severity:'critical',text:'Domain boundary registry must be valid JSON.',mock_approved:false}); }
  }
  // Check for broken regex replacement ($1..$9 used when regex has only non-capturing groups or 0 capturing groups)
  const replaceRegexMatches = content.matchAll(/\.replace\(\s*\/((?:\\\/|[^/\n])+)\/[a-z]*\s*,\s*(['"`])((?:\\.|(?!\2).)*)\2\s*\)/g);
  for (const m of replaceRegexMatches) {
    const regexBody = m[1];
    const replacementStr = m[3];
    if (/\$[1-9]\b/.test(replacementStr)) {
      const strippedEscapes = regexBody.replace(/\\./g, '').replace(/\[[^\]]*\]/g, '');
      const capturingGroups = (strippedEscapes.match(/\((?!\?)/g) || []).length;
      if (capturingGroups === 0) {
        findings.push({
          file,
          line: 0,
          pattern: 'broken_regex_capture_replacement',
          severity: 'critical',
          text: `Broken regex capture replacement: replacement string '${replacementStr}' references $1..$9 but regex /${regexBody}/ has 0 capturing groups.`,
          mock_approved: false
        });
      }
    }
  }
  // Check for silent error-swallowing catch blocks inside verification/validation modules
  if (/(?:validation|verifier|verify|audit|lint)/i.test(file)) {
    const emptyCatchRegex = /catch\s*(?:\([^)]*\))?\s*\{\s*(?:\/\/[^\n]*\s*|\/\*[\s\S]*?\*\/\s*)*\}/g;
    if (emptyCatchRegex.test(content)) {
      findings.push({
        file,
        line: 0,
        pattern: 'silent_validation_catch',
        severity: 'critical',
        text: `Silent empty catch block detected in verification/validation module '${file}'. Swallowing errors in validators causes false-positive passes.`,
        mock_approved: false
      });
    }
  }
  const isTestFile = /\.test\./.test(file) || /\.spec\./.test(file) || file.includes('/tests/') || file.includes('/__tests__/');

  // Check sealed oracle block integrity (applies to test files and any file with sealed oracle)
  if (isTestFile || content.includes('@iwish-sealed-oracle')) {
    checkSealedOracleIntegrity(file, content);
    if (isTestFile) {
      // AST mock hygiene check on test files (R12-04 & security-boundary mock guard)
      const prismaMockRegex = /(?:vi\.mock|jest\.mock)\s*\(\s*['"](?:@\/lib\/prisma|(?:\.\.?\/)+lib\/prisma)['"]|prisma\.[a-zA-Z0-9_]+\.(?:findMany|findUnique|findFirst|create|update|delete)\s*=\s*(?:vi\.fn|jest\.fn)/g;
      if (prismaMockRegex.test(content) || content.includes('mockPrisma') || content.includes('__mock_db_state')) {
        findings.push({
          file,
          line: 0,
          pattern: 'test_mock_hygiene_violation',
          severity: 'critical',
          text: 'MOCK_HYGIENE_VIOLATION: Direct mocking of prisma.<model>.* or fake DB state in test is forbidden (R12-04).',
          mock_approved: false
        });
        allMocksApproved = false;
        mockCount++;
      }
      const secBoundaryMockRegex = /(?:vi\.mock|jest\.mock)\s*\(\s*['"]@\/lib\/(?:prisma-rls|llm\/byok-cache|api-guard|api-guard-session)['"]/g;
      const isRouteOrIntegrationTest = file.includes('/api/') || file.includes('/tests/integration/') || /\.route\.test\./.test(file);
      if (secBoundaryMockRegex.test(content)) {
        findings.push({
          file,
          line: 0,
          pattern: 'test_security_boundary_mock',
          severity: 'critical',
          text: `SECURITY_BOUNDARY_MOCK: Mocking RLS/BYOK/Auth guard (@/lib/prisma-rls, byok-cache, api-guard) in '${file}' conceals production fail-open/cipher defects.`,
          mock_approved: false
        });
        allMocksApproved = false;
        mockCount++;
      }
      const fetchMockRegex = /(?:globalThis\.fetch|window\.fetch|vi\.spyOn\(\s*globalThis\s*,\s*['"]fetch['"]\s*\)|vi\.stubGlobal\(\s*['"]fetch['"])/;
      if ((isRouteOrIntegrationTest || file.includes('src/plugins/')) && fetchMockRegex.test(content) && /['"`]\/api\//.test(content)) {
        findings.push({
          file,
          line: 0,
          pattern: 'test_internal_api_fetch_mock',
          severity: 'critical',
          text: `PHANTOM_ROUTE_MOCK: Mocking globalThis.fetch against internal '/api/...' URLs in '${file}' hides 404/non-existent backend endpoints.`,
          mock_approved: false
        });
        allMocksApproved = false;
        mockCount++;
      }
      // Skip production anti-cheat checks on test files
      continue;
    }
  }

  // Check for magic test-ID branching in production API routes
  if (file.includes('src/app/api/') && !isTestFile) {
    const magicTestIdRegex = /(?:packageId|operationId|rawPackageId)\s*===\s*['"](?:pkg-not-found|non-existent-pkg|op-not-found)['"]/g;
    if (magicTestIdRegex.test(content)) {
      findings.push({
        file,
        line: 0,
        pattern: 'magic_test_id_backdoor',
        severity: 'critical',
        text: `Magic test-ID backdoor detected in production API route '${file}'. Use real DB lookups instead of string matching test IDs.`,
        mock_approved: false
      });
      allMocksApproved = false;
      mockCount++;
    }
  }

  const fileMocksApproved = /\[MOCK_APPROVED\]/i.test(content);
  const lines = content.split('\n');

  // Check console.log count
  const consoleLogMatches = content.match(/console\.log\(/g) || [];
  if (consoleLogMatches.length > 3) {
    findings.push({
      file,
      line: 0,
      pattern: 'console_log',
      severity: 'medium',
      text: `Excessive console.log found (${consoleLogMatches.length} occurrences)`,
      mock_approved: fileMocksApproved
    });
  }

  lines.forEach((line, index) => {
    const lineNumber = index + 1;

    // 1. Auth mock checks (Critical, CANNOT be approved)
    if (authMockPattern.test(line)) {
      findings.push({
        file,
        line: lineNumber,
        pattern: 'auth_mock',
        severity: 'critical',
        text: line.trim(),
        mock_approved: false // Auth mocks are never approvable
      });
      allMocksApproved = false;
      mockCount++;
    }

    // 2. Empty mock stubs (Critical)
    else if (emptyStubPatterns.some(p => p.test(line))) {
      findings.push({
        file,
        line: lineNumber,
        pattern: 'empty_stub',
        severity: 'critical',
        text: line.trim(),
        mock_approved: fileMocksApproved
      });
      if (!fileMocksApproved) allMocksApproved = false;
      mockCount++;
    }

    // 3. setTimeout simulation (High)
    else if (setTimeoutSimulate.test(line)) {
      findings.push({
        file,
        line: lineNumber,
        pattern: 'setTimeout_simulate',
        severity: 'high',
        text: line.trim(),
        mock_approved: fileMocksApproved
      });
      if (!fileMocksApproved) allMocksApproved = false;
      mockCount++;
    }

    // 4. Fake data patterns (High)
    else if (fakeDataPattern.test(line)) {
      findings.push({
        file,
        line: lineNumber,
        pattern: 'fake_data',
        severity: 'high',
        text: line.trim(),
        mock_approved: fileMocksApproved
      });
      if (!fileMocksApproved) allMocksApproved = false;
      mockCount++;
    }

    // 5. TODO markers (High)
    else if (todoPattern.test(line)) {
      findings.push({
        file,
        line: lineNumber,
        pattern: 'todo_marker',
        severity: 'high',
        text: line.trim(),
        mock_approved: fileMocksApproved
      });
    }
  });

  // ━━━ 5b. WORKER/PARSER/SECURITY ANTI-CHEAT PATTERNS (Epic 76 Hardening) ━━━
  if (!isTestFile && !file.includes('anti-cheat-linter.js') && /\.(?:ts|tsx|js|jsx)$/.test(file)) {
    const envBypassRe = /\bprocess\.env\.(?:NODE_ENV\s*!==?\s*['"]production['"]|NODE_ENV\s*===?\s*['"]test['"]|VITEST\b)/;
    const constBypassRe = /(?:===?\s*['"][^'"]+['"]|\w+)\s*\|\|\s*true\b|\|\|\s*true\s*(?:;|\)|\})|&&\s*false\b/;
    const decryptCatchFallbackRe = /function\s+[a-zA-Z0-9_]*(?:decrypt|unseal)[a-zA-Z0-9_]*[\s\S]{0,1200}catch\s*(?:\([^)]*\))?\s*\{[^}]*\breturn\s+(?:encrypted[A-Za-z0-9_]*|cipher[A-Za-z0-9_]*|stored[A-Za-z0-9_]*|raw[A-Za-z0-9_]*|[a-zA-Z0-9_]*\.encryptedKey)\b/;
    const fakeUtf8OcrRe = /(?:parseWithPlaintext|extractLocalDocument|ocr|pdf|docx)[a-zA-Z0-9_]*[\s\S]{0,600}\.toString\(\s*['"]utf-?8['"]\s*\)|\.toString\(\s*['"]utf-?8['"]\s*\)\s*(?:\.\s*replace|[\s\S]{0,200}\.replace\(\s*\/[^\n/]*\\x20-\\x7)/i;

    lines.forEach((line, index) => {
      const ln = index + 1;
      if (line.trim().startsWith('//') || line.trim().startsWith('*')) return;
      if (envBypassRe.test(line)) {
        findings.push({
          file,
          line: ln,
          pattern: 'env_test_bypass_in_prod',
          severity: 'critical',
          text: `Production environment/test bypass forbidden: ${line.trim()}`,
          mock_approved: false
        });
        allMocksApproved = false;
        mockCount++;
      }
      if (constBypassRe.test(line)) {
        findings.push({
          file,
          line: ln,
          pattern: 'constant_boolean_bypass',
          severity: 'critical',
          text: `Constant truthy/falsy boolean condition bypass forbidden: ${line.trim()}`,
          mock_approved: false
        });
        allMocksApproved = false;
        mockCount++;
      }
    });

    if (decryptCatchFallbackRe.test(content)) {
      findings.push({
        file,
        line: 0,
        pattern: 'plaintext_decrypt_fallback',
        severity: 'critical',
        text: `Plaintext/encryptedKey fallback inside decrypt catch block detected in '${file}'.`,
        mock_approved: false
      });
      allMocksApproved = false;
      mockCount++;
    }
    if (fakeUtf8OcrRe.test(content)) {
      findings.push({
        file,
        line: 0,
        pattern: 'fake_utf8_ocr_parser',
        severity: 'critical',
        text: `Raw UTF-8 Buffer coercion/stripper masquerading as OCR/document parser detected in '${file}'.`,
        mock_approved: false
      });
      allMocksApproved = false;
      mockCount++;
    }
  }

  // ━━━ 6. INLINE PRODUCTION MOCK DETECTION (Decision 6 — Party-Mode 2026-09-27) ━━━
  // "Malignant Stubs" defense: detects inline hardcoded domain data in .tsx files
  // without @mock-boundary annotation. Per-file aggregate counting.
  const isTsxOrTsComponent = file.endsWith('.tsx') || file.endsWith('.ts');
  // Load legacy mock baseline for Phase 1 grace period (Decision 3)
  const baselinePath = path.resolve('.agent/skills/codebase-drift-auditor/legacy-mock-baseline.json');
  let legacyFiles = new Set();
  if (fs.existsSync(baselinePath)) {
    try {
      const raw = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
      legacyFiles = new Set((raw.baseline_files || []).map(b => path.normalize(b.file)));
    } catch (e) {
      console.warn(`[anti-cheat-linter] failed loading legacy-mock-baseline.json (${baselinePath}): ${e?.message || e}`);
    }
  }

  const normalizedPath = path.normalize(file);
  const isLegacy = legacyFiles.has(normalizedPath) || Array.from(legacyFiles).some(lf => normalizedPath.endsWith(lf));
  const isExcluded = /\.(constants|config|theme|stories|storybook)\.(ts|tsx)$/.test(file);
  const hasMockBoundary = /@mock-boundary:/.test(content);

  if (isTsxOrTsComponent && !isExcluded) {
    const scriptKind = file.endsWith('.tsx') ? ts.ScriptKind.TSX : file.endsWith('.jsx') ? ts.ScriptKind.JSX : ts.ScriptKind.TS;
    let sf = null;
    try {
      sf = ts.createSourceFile(file, contentNoStyles, ts.ScriptTarget.Latest, true, scriptKind);
    } catch {
      sf = null;
    }

    // C4 Patch 0.6: Ban new Map()/new Set() top-level state persistence in backend/services
    // Allows local in-function variables (e.g. allowedIds = new Set()), static ReadonlySet/lookup constants, and algorithmic markers
    if (sf) {
      const isInsideFunctionScope = (node) => {
        let curr = node.parent;
        while (curr) {
          if (
            ts.isFunctionDeclaration(curr) ||
            ts.isFunctionExpression(curr) ||
            ts.isArrowFunction(curr) ||
            ts.isMethodDeclaration(curr) ||
            ts.isConstructorDeclaration(curr) ||
            ts.isGetAccessor(curr) ||
            ts.isSetAccessor(curr) ||
            (ts.isClassStaticBlockDeclaration && ts.isClassStaticBlockDeclaration(curr))
          ) {
            return true;
          }
          curr = curr.parent;
        }
        return false;
      };

      const isStaticConstantLookup = (varDecl, newExpr) => {
        const isConst = varDecl.parent && ts.isVariableDeclarationList(varDecl.parent) && (varDecl.parent.flags & ts.NodeFlags.Const) !== 0;
        if (!isConst) return false;

        const exprName = newExpr.expression.getText(sf);
        if (exprName === 'Set' && newExpr.arguments && newExpr.arguments.length === 1) {
          const arg = newExpr.arguments[0];
          if (ts.isArrayLiteralExpression(arg)) {
            return true;
          }
        }
        return false;
      };

      const visitMapSet = (node) => {
        if (ts.isNewExpression(node) && node.expression) {
          const exprName = node.expression.getText(sf);
          if (exprName === 'Map' || exprName === 'Set') {
            let varDecl = node.parent;
            while (varDecl && !ts.isVariableDeclaration(varDecl) && !ts.isStatement(varDecl)) {
              varDecl = varDecl.parent;
            }
            if (varDecl && ts.isVariableDeclaration(varDecl)) {
              const varName = varDecl.name.getText(sf);
              const lowerName = varName.toLowerCase();
              const inFunc = isInsideFunctionScope(varDecl);
              const isAlgorithmic = lowerName.includes('memo') || lowerName.includes('visited') ||
                lowerName.includes('cache') || lowerName.includes('dedup') || lowerName.includes('allowed') ||
                lowerName.includes('shared') || content.includes('@transient-algorithm');
              const isStaticConst = isStaticConstantLookup(varDecl, node);
              const isMutated = new RegExp(`\\b${varName}\\.(?:set|add|delete|clear)\\s*\\(`).test(content);

              if (!inFunc && !isAlgorithmic && (!isStaticConst || isMutated)) {
                const { line: lineNum } = sf.getLineAndCharacterOfPosition(varDecl.getStart(sf));
                findings.push({
                  file,
                  line: lineNum + 1,
                  pattern: 'in_memory_persistence',
                  severity: 'critical',
                  text: `Top-level in-memory persistence detected: 'new Map()' or 'new Set()' (var: ${lowerName}) is forbidden as a module data store. Use Prisma/Redis instead, or annotate with @transient-algorithm.`,
                  mock_approved: false
                });
                allMocksApproved = false;
                mockCount++;
              }
            }
          }
        }
        ts.forEachChild(node, visitMapSet);
      };
      visitMapSet(sf);
    } else {
      const lines = content.split('\n');
      lines.forEach((lineText, idx) => {
        const match = lineText.match(/(?:const|let|var)\s+(\w+)\s*=\s*new\s+(Map|Set)\s*\(/);
        if (match) {
          const varName = match[1];
          const lowerName = varName.toLowerCase();
          const isIndented = /^\s{2,}/.test(lineText);
          const isStaticConstSet = /const\s+[A-Z0-9_]+\s*=\s*new\s+Set\s*\(\s*\[/.test(lineText);
          const isAlgorithmic = isIndented || isStaticConstSet || lowerName.includes('memo') || lowerName.includes('visited') ||
            lowerName.includes('cache') || lowerName.includes('dedup') || lowerName.includes('allowed') ||
            lowerName.includes('shared') || lineText.includes('@transient-algorithm') || content.includes('@transient-algorithm');
          if (!isAlgorithmic) {
            findings.push({
              file,
              line: idx + 1,
              pattern: 'in_memory_persistence',
              severity: 'critical',
              text: `Top-level in-memory persistence detected: 'new Map()' or 'new Set()' (var: ${lowerName}) is forbidden as a module data store. Use Prisma/Redis instead, or annotate with @transient-algorithm.`,
              mock_approved: false
            });
            allMocksApproved = false;
            mockCount++;
          }
        }
      });
    }

    // EC-P11-03: Strict path-bounding for lucide-react (Shadcn UI only)
    if (content.includes('lucide-react') && !normalizedPath.includes('/components/ui/')) {
        findings.push({
            file,
            line: 0,
            pattern: 'illegal_external_icon',
            severity: 'critical',
            text: `External icon library 'lucide-react' is forbidden outside '/components/ui/'. Use <BrandIcon name="..."> instead.`,
            mock_approved: false
        });
        allMocksApproved = false;
        mockCount++;
    }
    
    // EC-P11-01: GuardedRoute must be strictly imported from @/lib/api-guard
    if (file.includes('src/app/api/') && content.includes('guardedRoute')) {
        if (!content.match(/import\s+\{.*guardedRoute.*\}\s+from\s+['"]@\/lib\/api-guard['"]/)) {
            findings.push({
                file,
                line: 0,
                pattern: 'hallucinated_guarded_route',
                severity: 'critical',
                text: `Security Violation: 'guardedRoute' is used but not imported strictly from '@/lib/api-guard'. Hallucination or override detected.`,
                mock_approved: false
            });
            allMocksApproved = false;
        }
    }

    // Strip React JSX inline style blocks style={{ ... }} so CSS styles are not counted as domain mock data
    const contentNoStyles = content.replace(/style=\{\{[\s\S]*?\}\}/g, '');
    // Strip TypeScript interface/type declarations so type fields (`field: true | false | 'literal'`) are not counted as object literal properties
    const contentNoTypes = contentNoStyles
      .replace(/\binterface\s+\w+(?:\s*<[^>]*>)?\s*\{[\s\S]*?\n\}/gm, '')
      .replace(/\btype\s+\w+(?:\s*<[^>]*>)?\s*=\s*\{[\s\S]*?\n\};?/gm, '');
    // FIX R2-P6 (RPN=80): Only strip FULL-LINE comments (^\\s*//) to preserve URLs in strings
    const contentNoComments = contentNoTypes.replace(/^\s*\/\/.*$/gm, '').replace(/\/\*[\s\S]*?\*\//g, '');
    // PIPE-HARDEN-01 v3.1: Only count type erasure in assignment/cast contexts (not function params or safe double-casts `as unknown as Type` or optional Prisma model checks `(prisma as any)`)
    const typeErasureMatches = contentNoComments.match(/(?<!\(\s*prisma\s+)\bas\s+any\b|\bas\s+unknown\b(?!\s+as\b)|=\s*\w+\s*:\s*any\b/g) || [];
    // Scope property count per individual ObjectLiteralExpression subtree instead of summing across the whole file
    let maxObjectLiteralProps = 0;
    try {
      const scriptKind = file.endsWith('.tsx') ? ts.ScriptKind.TSX : file.endsWith('.jsx') ? ts.ScriptKind.JSX : ts.ScriptKind.TS;
      const sf = ts.createSourceFile(file, contentNoStyles, ts.ScriptTarget.Latest, true, scriptKind);
      const isDbQueryCall = (callNode) => {
        const exprText = callNode.expression.getText(sf);
        return /(?:^|\.)(?:tx|prisma)\./.test(exprText);
      };
      const countSubtreeDataProps = (objNode) => {
        let count = 0;
        const walk = (curr, isRoot) => {
          if (!isRoot && ts.isFunctionLike(curr)) {
            return;
          }
          if (ts.isCallExpression(curr) && isDbQueryCall(curr)) {
            return;
          }
          if (ts.isShorthandPropertyAssignment(curr)) {
            count++;
          } else if (ts.isPropertyAssignment(curr)) {
            if (!curr.initializer || !ts.isFunctionLike(curr.initializer)) {
              count++;
            }
          }
          ts.forEachChild(curr, (child) => walk(child, false));
        };
        walk(objNode, true);
        return count;
      };
      const visitObj = (node) => {
        if (ts.isCallExpression(node) && isDbQueryCall(node)) {
          return;
        }
        if (ts.isObjectLiteralExpression(node)) {
          const propCount = countSubtreeDataProps(node);
          if (propCount > maxObjectLiteralProps) {
            maxObjectLiteralProps = propCount;
          }
        }
        ts.forEachChild(node, visitObj);
      };
      visitObj(sf);
    } catch (err) {
      process.stderr.write(`[anti-cheat-linter] Failed to parse ${file}: ${err?.message || err}\n`);
      maxObjectLiteralProps = 0;
    }
    const aggregateProps = maxObjectLiteralProps;

    // Check for large inline domain data literal: matches an object/array assignment with >15 inner properties
    const domainDataBlocks = contentNoStyles.match(/const\s+(\w*(?:data|mock|dummy|conflict|reconcil|order|user|metric|item|result|response|payload)\w*)\s*(?::\s*[^=]+)?=\s*([{[][\s\S]*?;\s*(?:\n|$))/gi) || [];
    let hasLargeDomainDataBlock = false;
    for (const block of domainDataBlocks) {
      // Exclude blocks that invoke Prisma or database queries (`await tx.`, `await prisma.`)
      if (/\b(?:await\s+)?(?:tx|prisma)\.[a-zA-Z0-9_]+\./.test(block)) {
        continue;
      }
      // Exclude small UI menu/tab/nav/action constants (e.g. NEW_TAB_MENU_ITEMS, TAB_ITEMS, MENU_ITEMS)
      const varNameMatch = block.match(/const\s+(\w+)/);
      const varName = varNameMatch ? varNameMatch[1] : '';
      const isUiMenuConstant = /(?:menu|tab|nav|toolbar|action|filter)_items?/i.test(varName) ||
        (/\b(?:icon|labelKey|defaultLabel|badge)\s*:/i.test(block) && !/\b(?:tenantId|userId|projectId|price|cost|amount|password|secret)\b/i.test(block));
      if (isUiMenuConstant) {
        continue;
      }

      const blockProps = (block.match(/\w+\s*:\s*(?:['"`\d[{]|true\b|false\b|null\b|undefined\b|new\s)/gm) || []).length;
      if (blockProps > 15) {
        hasLargeDomainDataBlock = true;
        break;
      }
    }
    // v3.1: Only trigger on assignment-level type erasure (not function param annotations)
    const isMalignantStub = hasLargeDomainDataBlock || (typeErasureMatches.length > 0 && aggregateProps > 25);

    if (isMalignantStub && !hasMockBoundary) {
      if (isLegacy) {
        findings.push({
          file,
          line: 0,
          pattern: 'inline_production_mock_legacy',
          severity: 'warning',
          text: `[Legacy Baseline Warning] Inline mock detected: ${aggregateProps} properties. Scheduled for burn-down removal.`,
          mock_approved: true
        });
      } else {
        findings.push({
          file,
          line: 0,
          pattern: 'inline_production_mock',
          severity: 'critical',
          text: `Inline production mock detected: ${aggregateProps} aggregate object properties without @mock-boundary annotation. Use Fishery factories or annotate with // @mock-boundary: <Story-ID> expires: <date>`,
          mock_approved: false
        });
        allMocksApproved = false;
        mockCount++;
      }
    }

    // Validate @mock-boundary expires date if present
    if (hasMockBoundary) {
      const expiresMatch = content.match(/@mock-boundary:.*?expires:\s*(\d{4}-\d{2}-\d{2})/);
      if (!expiresMatch) {
        findings.push({
          file,
          line: 0,
          pattern: 'mock_boundary_no_expires',
          severity: 'critical',
          text: `@mock-boundary found but missing 'expires: YYYY-MM-DD'. Add expiration date (max 14 days from commit).`,
          mock_approved: false
        });
        allMocksApproved = false;
        mockCount++;
      } else {
        const expiresDate = new Date(expiresMatch[1]);
        const now = new Date();
        // FIX R2-P6 (RPN=125): Guard against NaN from invalid dates like 9999-99-99
        // In JS, NaN comparisons always return false, silently bypassing all checks
        if (isNaN(expiresDate.getTime())) {
          findings.push({
            file,
            line: 0,
            pattern: 'mock_boundary_invalid_date',
            severity: 'critical',
            text: `@mock-boundary has invalid expires date: '${expiresMatch[1]}'. Must be valid YYYY-MM-DD format.`,
            mock_approved: false
          });
          allMocksApproved = false;
          mockCount++;
        } else if (expiresDate < now) {
          findings.push({
            file,
            line: 0,
            pattern: 'mock_boundary_expired',
            severity: 'critical',
            text: `@mock-boundary expired on ${expiresMatch[1]}. Remove mock data or extend via /extend-mock-boundary.`,
            mock_approved: false
          });
          allMocksApproved = false;
          mockCount++;
        } else {
          // Enforce max 14-day expiry window — prevents gaming via expires: 2099-12-31
          const maxExpiryMs = 14 * 24 * 60 * 60 * 1000; // 14 days in ms
          if (expiresDate.getTime() - now.getTime() > maxExpiryMs) {
            findings.push({
              file,
              line: 0,
              pattern: 'mock_boundary_too_far',
              severity: 'critical',
              text: `@mock-boundary expires too far in future (${expiresMatch[1]}). Max allowed: 14 days from today. Use a closer date or request extension via /extend-mock-boundary.`,
              mock_approved: false
            });
            allMocksApproved = false;
            mockCount++;
          }
        }
      }
    }

    // Type-erasure in .tsx without mock boundary
    if (typeErasureMatches.length > 0 && aggregateProps > 10 && !hasMockBoundary) {
      if (isLegacy) {
        findings.push({
          file,
          line: 0,
          pattern: 'type_erasure_mock_legacy',
          severity: 'warning',
          text: `[Legacy Baseline Warning] Type erasure (${typeErasureMatches.length}x) combined with ${aggregateProps} inline properties. Scheduled for burndown.`,
          mock_approved: true
        });
      } else {
        findings.push({
          file,
          line: 0,
          pattern: 'type_erasure_mock',
          severity: 'high',
          text: `Type erasure (${typeErasureMatches.length}x) combined with ${aggregateProps} inline properties suggests undocumented mock data.`,
          mock_approved: fileMocksApproved
        });
        if (!fileMocksApproved) allMocksApproved = false;
        mockCount++;
      }
    }
  }
}

// ━━━ ORACLE DISPUTE CHECK (R10-08 / EXIT 17) ━━━
let hasActiveDispute = false;
let disputePathFound = null;

const possibleDisputePaths = [];
if (storyDir) {
  if (storyId) {
    possibleDisputePaths.push(path.join(storyDir, `oracle-dispute-${storyId}.json`));
    possibleDisputePaths.push(path.join(storyDir, `oracle-dispute-${storyId.replace(/\./g, '-')}.json`));
  }
  try {
    if (fs.existsSync(storyDir)) {
      const storyFiles = fs.readdirSync(storyDir);
      for (const sf of storyFiles) {
        if (sf.startsWith('oracle-dispute-') && sf.endsWith('.json')) {
          possibleDisputePaths.push(path.join(storyDir, sf));
        }
      }
    }
  } catch (err) {
    console.warn(`[anti-cheat-linter] failed scanning oracle disputes in ${storyDir}: ${err?.message || err}`);
  }
}

const scratchDir = path.resolve(process.cwd(), '_iwish-output/adhoc-workspace/scratch');
if (fs.existsSync(scratchDir)) {
  if (storyId) {
    possibleDisputePaths.push(path.join(scratchDir, `oracle-dispute-${storyId}.json`));
    possibleDisputePaths.push(path.join(scratchDir, `oracle-dispute-${storyId.replace(/\./g, '-')}.json`));
  }
  try {
    const scratchFiles = fs.readdirSync(scratchDir);
    for (const sf of scratchFiles) {
      if (sf.startsWith('oracle-dispute-') && sf.endsWith('.json')) {
        if (!storyId || sf.includes(storyId) || sf.includes(storyId.replace(/\./g, '-'))) {
          possibleDisputePaths.push(path.join(scratchDir, sf));
        }
      }
    }
  } catch (err) {
    console.warn(`[anti-cheat-linter] failed scanning oracle disputes in ${scratchDir}: ${err?.message || err}`);
  }
}

for (const p of [...new Set(possibleDisputePaths)]) {
  if (fs.existsSync(p)) {
    try {
      const disputeContent = JSON.parse(fs.readFileSync(p, 'utf8'));
      if (disputeContent.status === 'ORACLE_DISPUTE_OPENED' || disputeContent.error_output) {
        hasActiveDispute = true;
        disputePathFound = p;
        findings.push({
          file: p,
          line: 0,
          pattern: 'oracle_dispute_active',
          severity: 'critical',
          text: `ORACLE_DISPUTE_OPENED: Story ${disputeContent.story_id || storyId} has an active oracle dispute pending resolution. Exit 17 invoked.`,
          mock_approved: false
        });
        break;
      }
    } catch (err) {
      console.warn(`[anti-cheat-linter] failed reading oracle dispute file ${p}: ${err?.message || err}`);
    }
  }
}

// Build output JSON
const outputData = {
  story_id: storyId || 'unknown',
  timestamp: new Date().toISOString(),
  script_version: '3.1.0', // v3.1: Added sealed oracle integrity and oracle dispute handling (Exit 17)
  files_scanned: files.length,
  total_findings: findings.length + failures.length,
  mock_count: mockCount,
  all_mocks_approved: allMocksApproved,
  oracle_dispute: hasActiveDispute,
  findings: [
    ...failures.map(f => ({
      file: 'config',
      line: 0,
      pattern: 'config_failure',
      severity: 'critical',
      text: f,
      mock_approved: false
    })),
    ...findings
  ],
  exit_code: 0
};

// Check if we should exit with 1 or 17
const criticalFailures = outputData.findings.filter(f => 
  f.severity === 'critical' || 
  (f.severity === 'high' && !f.mock_approved)
);

if (hasActiveDispute) {
  outputData.exit_code = 17;
} else if (criticalFailures.length > 0) {
  outputData.exit_code = 1;
}

// Write JSON artifact if storyDir and storyId are provided
if (storyDir && storyId) {
  if (!fs.existsSync(storyDir)) {
    fs.mkdirSync(storyDir, { recursive: true });
  }
  const outputPath = path.join(storyDir, `linter-output-${storyId}.json`);
  fs.writeFileSync(outputPath, JSON.stringify(outputData, null, 2), 'utf8');
  console.log(`Linter results written to: ${outputPath}`);
}

if (outputData.exit_code === 17) {
  console.error(`\n❌ [ORACLE DISPUTE] Anti-cheat linter halted: Active Oracle Dispute detected (Exit 17).`);
  if (disputePathFound) console.error(`  Dispute recorded at: ${disputePathFound}`);
  process.exit(17);
}

if (outputData.exit_code === 1) {
  console.error(`\n❌ Anti-cheat linter failed with ${criticalFailures.length} blocking issues:`);
  criticalFailures.forEach(f => {
    console.error(`  - [${f.severity.toUpperCase()}] ${f.file}:${f.line} -> ${f.text}`);
  });
  process.exit(1);
}

console.log('\n✅ Anti-cheat linter passed.');
process.exit(0);
