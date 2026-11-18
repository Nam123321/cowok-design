// @story 08.2
// @implements AC-04, AC-07
import { VideoStoryboardReceipt, VideoStoryboardReceiptSchema } from "./types";
import { invokePluginHostBridge } from "./repackager-bundle";

/** Strict fail-closed storyboard parser delegating to authoritative Rust `VideoIntakeProcessor` IPC (`[frame: mm:ss.xx] -> Action: Intent`, AC-07). */
export function parseVideoStoryboardStrict(videoSource: string, rawLog: string): VideoStoryboardReceipt {
  const raw = invokePluginHostBridge<unknown>("video-intake", {
    video_source: videoSource,
    raw_log: rawLog,
  });
  return VideoStoryboardReceiptSchema.parse(raw);
}

export function parseVideoStoryboard(videoSource: string, rawLog: string): VideoStoryboardReceipt {
  return parseVideoStoryboardStrict(videoSource, rawLog);
}
