#!/usr/bin/env node
import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const BASELINE_PATH = path.resolve(__dirname, '../.github/security/audit-baseline.json');

/**
 * Evaluates the repository pnpm audit output against the security ratchet baseline.
 * Supports dependency injection for testing.
 * @param {string} [rawAuditOutput] Optional injected audit raw output
 * @param {{ max_critical: number, max_high: number }} [customBaseline] Optional injected baseline
 * @returns {{ success: boolean, critical: number, high: number }}
 */
export function evaluateAuditRatchet(rawAuditOutput, customBaseline) {
  console.log('🔍 Running Dependency Vulnerability Ratchet (SCA)...');
  let baseline = customBaseline || { max_critical: 3, max_high: 2 };
  if (!customBaseline && fs.existsSync(BASELINE_PATH)) {
    try {
      baseline = JSON.parse(fs.readFileSync(BASELINE_PATH, 'utf8'));
    } catch (err) {
      console.warn('⚠️ Could not parse audit-baseline.json, using default fallback limits:', err.message);
    }
  }

  let auditJson = rawAuditOutput;
  if (auditJson === undefined) {
    try {
      auditJson = execSync('pnpm audit --audit-level=high --json', {
        encoding: 'utf8',
        stdio: ['pipe', 'pipe', 'pipe'],
        maxBuffer: 50 * 1024 * 1024,
      });
    } catch (err) {
      auditJson = err.stdout ? err.stdout.toString() : '';
      if (!auditJson && err.stderr) {
        console.error('::error::pnpm audit failed to execute:', err.stderr.toString());
        return { success: false, critical: 999, high: 999 };
      }
    }
  }

  if (rawAuditOutput === '') {
    return { success: true, critical: 0, high: 0 };
  }

  if (!auditJson || !auditJson.trim()) {
    console.error('::error::🚨 Security Ratchet Error: pnpm audit returned empty output. Failing closed.');
    return { success: false, critical: 999, high: 999 };
  }

  let criticalCount = 0;
  let highCount = 0;
  let parsedFromJson = false;

  try {
    const parsed = JSON.parse(auditJson);
    if (parsed.metadata && parsed.metadata.vulnerabilities) {
      criticalCount = parsed.metadata.vulnerabilities.critical || 0;
      highCount = parsed.metadata.vulnerabilities.high || 0;
      parsedFromJson = true;
    }
  } catch (err) {
    console.warn('⚠️ Direct JSON parse failed, falling back to regex matching:', err.message);
  }

  if (!parsedFromJson) {
    const criticalMatches = auditJson.match(/"critical":\s*(\d+)/g);
    if (criticalMatches && criticalMatches.length > 0) {
      const last = criticalMatches[criticalMatches.length - 1];
      criticalCount = parseInt(last.replace(/[^0-9]/g, ''), 10) || 0;
      parsedFromJson = true;
    }

    const highMatches = auditJson.match(/"high":\s*(\d+)/g);
    if (highMatches && highMatches.length > 0) {
      const last = highMatches[highMatches.length - 1];
      highCount = parseInt(last.replace(/[^0-9]/g, ''), 10) || 0;
      parsedFromJson = true;
    }
  }

  if (!parsedFromJson) {
    console.error('::error::🚨 Security Ratchet Error: Could not parse vulnerability counts from pnpm audit output. Failing closed.');
    return { success: false, critical: 999, high: 999 };
  }

  console.log(`📊 Audit Scan Results: Critical: ${criticalCount} (Baseline: ${baseline.max_critical}), High: ${highCount} (Baseline: ${baseline.max_high})`);

  let failed = false;
  if (criticalCount > baseline.max_critical) {
    console.error(`::error::🚨 Security Ratchet Breach: Critical vulnerabilities (${criticalCount}) exceeded baseline (${baseline.max_critical})! New critical CVEs introduced.`);
    failed = true;
  }
  if (highCount > baseline.max_high) {
    console.error(`::error::🚨 Security Ratchet Breach: High vulnerabilities (${highCount}) exceeded baseline (${baseline.max_high})! New high CVEs introduced.`);
    failed = true;
  }

  return { success: !failed, critical: criticalCount, high: highCount };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  const result = evaluateAuditRatchet();
  if (!result.success) {
    process.exit(1);
  }
  console.log('✅ Dependency vulnerability count within approved ratchet baseline limits.');
  process.exit(0);
}
