import {
  ElementCommentResponse,
  StaleVerificationReport,
} from "../../../lib/validations/element-comment";
import { computeDomHash } from "./dom-hash";

export interface DomElementSnapshot {
  tagName: string;
  classNames: string;
  innerText: string;
}

export function verifyAnchorStatus(
  comment: ElementCommentResponse,
  snapshot: DomElementSnapshot | null
): StaleVerificationReport {
  if (!snapshot) {
    return {
      commentId: comment.id,
      elementFfId: comment.elementFfId,
      expectedDomHash: comment.domHash,
      actualDomHash: null,
      isStale: true,
      reason: "ELEMENT_MISSING",
    };
  }

  const currentHash = computeDomHash(
    snapshot.tagName,
    snapshot.classNames,
    snapshot.innerText
  );

  const isMatch = currentHash === comment.domHash;

  return {
    commentId: comment.id,
    elementFfId: comment.elementFfId,
    expectedDomHash: comment.domHash,
    actualDomHash: currentHash,
    isStale: !isMatch,
    reason: isMatch ? "SYNCHRONIZED" : "HASH_MISMATCH",
  };
}
