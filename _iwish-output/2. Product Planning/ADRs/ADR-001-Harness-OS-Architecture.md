# ADR-001: Architecture Decision Record — UI/UX OS as an Agent Harness Operating System

**Status:** ACCEPTED  
**Date:** 2026-11-02  
**Deciders:** AI Systems Architect, Solution Architect, QA Reviewer  

## 1. Context
Các AI Coding Agent khi thiết kế giao diện thường bị phân mảnh phong cách, ảo giác mã hex và gây tràn token khi nhồi PRD lớn vào prompt. Định dạng bài thuyết trình truyền thống (PPTX) không tương thích với Agent.

## 2. Decision
1. Xây dựng UI/UX OS dưới dạng một **Agent Harness Operating System** khép kín (quản lý Context, Tool Registry, Deterministic Evals).
2. Chuẩn hóa Bộ 9 Project Context Specs thay thế cho một file `DESIGN.md` đơn lẻ.
3. Áp dụng cơ chế JIT Dynamic Prompt Assembly tiết kiệm 80% token.
4. Mở rộng sang phân hệ HTML-Native Presentation & Dual-Pane CMS cho người dùng non-tech.
5. Cung cấp MCP Plugin chuẩn cho Cowok.ai.

## 3. Consequences
- Đảm bảo 100% không lệch chuẩn thương hiệu (zero design drift).
- Người dùng non-tech có thể tự sửa wording/số liệu trên slide mà không làm hỏng code.
