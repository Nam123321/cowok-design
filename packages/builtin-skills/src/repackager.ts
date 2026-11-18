// @story 08.2
// @implements AC-01, AC-02, AC-06, AC-07
import { RepackageReceipt, RepackageReceiptSchema } from "./types";
import {
  classifyAndValidateSourceAsset,
  scanExternalScriptSecurity,
  scanSkillMarkdownSafety,
} from "./repackager-security";
import { parseVideoStoryboard, parseVideoStoryboardStrict } from "./video-intake";
import {
  MAX_DISCOVERY_STUB_BYTES,
  MAX_SINGLE_FILE_BYTES,
  PKG_HASH_DOMAIN,
  StagedBundleReceipt,
  commitRepackagedDir,
  invokePluginHostBridge,
  manifestToSkillPlugin,
  stageRepackagedBundle,
  verifyRepackagedBundle,
} from "./repackager-bundle";

export type { StagedBundleReceipt };
export {
  MAX_DISCOVERY_STUB_BYTES,
  MAX_SINGLE_FILE_BYTES,
  PKG_HASH_DOMAIN,
  classifyAndValidateSourceAsset,
  commitRepackagedDir,
  manifestToSkillPlugin,
  parseVideoStoryboard,
  parseVideoStoryboardStrict,
  scanExternalScriptSecurity,
  scanSkillMarkdownSafety,
  stageRepackagedBundle,
  verifyRepackagedBundle,
};

export const MAX_TRAVERSAL_DEPTH = 8;
export const MAX_PACKAGE_FILES = 256;
export const MAX_TOTAL_PACKAGE_BYTES = 8388608;

/** Repackages an external skill directory into certified agentskills.io format via authoritative D2 kernel FD-relative engine (AC-01..AC-07). */
export function repackageDirectory(params: {
  sourceDir: string;
  destDir: string;
  skillName: string;
  description: string;
  capabilities: string[];
  upstreamUrl?: string;
}): RepackageReceipt {
  const { sourceDir, destDir, skillName, description, capabilities, upstreamUrl } = params;
  const raw = invokePluginHostBridge<unknown>("repackage", {
    source_dir: sourceDir,
    dest_dir: destDir,
    skill_name: skillName,
    description,
    capabilities,
    upstream_url: upstreamUrl ?? null,
  });
  return RepackageReceiptSchema.parse(raw);
}
