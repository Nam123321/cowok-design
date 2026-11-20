// @story 04.5, 05.1, 06.2
// @implements AC-05, AC-10 (Floating Overlay Pod Position & Viewport Clamp Utility)
const POD_MARGIN_PX = 8;
const POD_MIN_WIDTH_PX = 200;
const POD_DEFAULT_WIDTH_PX = 310;
const POD_DEFAULT_STAGE_WIDTH_PX = 420;
const POD_DEFAULT_STAGE_HEIGHT_PX = 640;
const POD_TARGET_MAX_OFFSET_PX = 80;
const POD_VERTICAL_GAP_PX = 8;
const POD_DEFAULT_EST_HEIGHT_PX = 180;

export function computeFloatingPodLayout(opts: {
  rect: { left: number; top: number; width: number; height: number };
  stageWidth?: number;
  stageHeight?: number;
  preferredWidth?: number;
  estimatedHeight?: number;
}): { left: number; top: number; width: number } {
  const availW = Math.max(POD_MIN_WIDTH_PX, (opts.stageWidth ?? POD_DEFAULT_STAGE_WIDTH_PX) - POD_MARGIN_PX * 2);
  const width = Math.min(opts.preferredWidth ?? POD_DEFAULT_WIDTH_PX, availW);
  const maxLeft = Math.max(POD_MARGIN_PX, (opts.stageWidth ?? POD_DEFAULT_STAGE_WIDTH_PX) - width - POD_MARGIN_PX);
  const left = Math.min(Math.max(POD_MARGIN_PX, opts.rect.left), maxLeft);

  const stageH = opts.stageHeight ?? POD_DEFAULT_STAGE_HEIGHT_PX;
  const estH = opts.estimatedHeight ?? POD_DEFAULT_EST_HEIGHT_PX;
  const belowTop = opts.rect.top + Math.min(opts.rect.height, POD_TARGET_MAX_OFFSET_PX) + POD_VERTICAL_GAP_PX;
  const maxTop = Math.max(POD_MARGIN_PX, stageH - estH - POD_MARGIN_PX);
  let top = belowTop;
  if (stageH > 0 && belowTop + estH > stageH - POD_MARGIN_PX) {
    const aboveTop = opts.rect.top - estH - POD_VERTICAL_GAP_PX;
    top = aboveTop >= POD_MARGIN_PX ? aboveTop : Math.min(Math.max(POD_MARGIN_PX, belowTop), maxTop);
  }
  return { left, top: Math.max(POD_MARGIN_PX, top), width };
}
