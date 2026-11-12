import { describe, expect, it } from "vitest";
import {
  AnchorStatusSchema,
  CreateElementCommentSchema,
  ElementCommentResponse,
  UpdateElementCommentSchema,
} from "../../../lib/validations/element-comment";
import { computeDomHash } from "../utils/dom-hash";
import { verifyAnchorStatus } from "../utils/stale-anchor-checker";
import { apiElementComment } from "../../../api/element-comment";
import { GET, POST } from "../../../app/api/projects/[id]/flow/nodes/[nodeId]/comments/route";

describe("Story 04.3: Element-Anchored Comments & Stale Anchor Detection", () => {
  describe("AC-01: Element-Anchored Comments & DOM Hash", () => {
    it("should compute deterministic DOM hash across whitespace variations", () => {
      const hash1 = computeDomHash("button", "btn btn-primary active", "Submit Order");
      const hash2 = computeDomHash("BUTTON", "active   btn   btn-primary", "  Submit Order  ");
      expect(hash1).toBe(hash2);
      expect(typeof hash1).toBe("string");
      expect(hash1.length).toBe(8);
    });

    it("should produce different hash for modified text or classes", () => {
      const baseHash = computeDomHash("button", "btn btn-primary", "Submit Order");
      const diffTextHash = computeDomHash("button", "btn btn-primary", "Cancel Order");
      const diffClassHash = computeDomHash("button", "btn btn-danger", "Submit Order");

      expect(baseHash).not.toBe(diffTextHash);
      expect(baseHash).not.toBe(diffClassHash);
    });

    it("should validate CreateElementCommentSchema correctly", () => {
      const valid = CreateElementCommentSchema.safeParse({
        elementFfId: "ff-btn-submit",
        author: "Lead Designer",
        content: "Please change the button variant to solid",
        domHash: "a1b2c3d4",
      });
      expect(valid.success).toBe(true);

      const emptyContent = CreateElementCommentSchema.safeParse({
        elementFfId: "ff-btn-submit",
        author: "Lead Designer",
        content: "",
        domHash: "a1b2c3d4",
      });
      expect(emptyContent.success).toBe(false);
    });

    it("should validate AnchorStatusSchema", () => {
      expect(AnchorStatusSchema.safeParse("active").success).toBe(true);
      expect(AnchorStatusSchema.safeParse("stale").success).toBe(true);
      expect(AnchorStatusSchema.safeParse("resolved").success).toBe(true);
      expect(AnchorStatusSchema.safeParse("deleted").success).toBe(false);
    });

    it("should instantiate apiElementComment client SDK with required methods", () => {
      expect(typeof apiElementComment.getComments).toBe("function");
      expect(typeof apiElementComment.createComment).toBe("function");
      expect(typeof apiElementComment.updateComment).toBe("function");
      expect(typeof apiElementComment.resolveComment).toBe("function");
    });
  });

  describe("AC-02: Fastify REST API Route Handlers", () => {
    it("should create and retrieve element-anchored comments via REST endpoints", async () => {
      const req = new Request("http://localhost/api/v1/projects/proj-1/flow/nodes/node-1/comments", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          elementFfId: "ff-nav-logo",
          author: "Product Manager",
          content: "Enlarge logo on mobile",
          domHash: "98765432",
        }),
      });

      const ctx = {
        projectId: "proj-1",
        tenantId: "tenant-1",
        params: { id: "proj-1", nodeId: "node-1" },
      };

      const postRes = await POST(req, ctx);
      expect(postRes.status).toBe(201);
      const comment = await postRes.json();
      expect(comment.elementFfId).toBe("ff-nav-logo");
      expect(comment.anchorStatus).toBe("active");

      const getReq = new Request("http://localhost/api/v1/projects/proj-1/flow/nodes/node-1/comments");
      const getRes = await GET(getReq, ctx);
      expect(getRes.status).toBe(200);
      const comments = await getRes.json();
      expect(comments.length).toBeGreaterThanOrEqual(1);
    });
  });

  describe("AC-03 [EDGE-CASE]: Stale Anchor Detection Engine", () => {
    const mockComment: ElementCommentResponse = {
      id: "cmt-1",
      tenantId: "tenant-1",
      projectId: "proj-1",
      nodeId: "node-1",
      elementFfId: "ff-card-header",
      author: "UX Auditor",
      content: "Header contrast ratio fails WCAG AA",
      domHash: computeDomHash("h3", "font-bold text-gray-400", "Analytics Dashboard"),
      anchorStatus: "active",
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };

    it("should flag ELEMENT_MISSING when target data-ff-id is deleted or moved", () => {
      const report = verifyAnchorStatus(mockComment, null);
      expect(report.isStale).toBe(true);
      expect(report.reason).toBe("ELEMENT_MISSING");
      expect(report.actualDomHash).toBeNull();
    });

    it("should flag HASH_MISMATCH when target element text or styles are modified", () => {
      const mutatedSnapshot = {
        tagName: "h3",
        classNames: "font-semibold text-gray-600",
        innerText: "Analytics Overview",
      };

      const report = verifyAnchorStatus(mockComment, mutatedSnapshot);
      expect(report.isStale).toBe(true);
      expect(report.reason).toBe("HASH_MISMATCH");
      expect(report.actualDomHash).not.toBe(mockComment.domHash);
    });

    it("should confirm SYNCHRONIZED when target element matches domHash", () => {
      const exactSnapshot = {
        tagName: "h3",
        classNames: "font-bold text-gray-400",
        innerText: "Analytics Dashboard",
      };

      const report = verifyAnchorStatus(mockComment, exactSnapshot);
      expect(report.isStale).toBe(false);
      expect(report.reason).toBe("SYNCHRONIZED");
      expect(report.actualDomHash).toBe(mockComment.domHash);
    });
  });
});
