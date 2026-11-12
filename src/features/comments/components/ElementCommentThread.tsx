import React from "react";
import { ElementCommentResponse } from "../../../lib/validations/element-comment";
import { ElementCommentItem } from "./ElementCommentItem";
import { NewCommentForm } from "./NewCommentForm";

interface ElementCommentThreadProps {
  comments: ElementCommentResponse[];
  nodeId: string;
  onAddComment?: (content: string, elementFfId: string) => void;
  onResolveComment?: (commentId: string) => void;
  onReanchorComment?: (commentId: string) => void;
}

export const ElementCommentThread: React.FC<ElementCommentThreadProps> = ({
  comments,
  onAddComment,
  onResolveComment,
  onReanchorComment,
}) => {
  return (
    <div
      className="element-comment-thread"
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "12px",
        width: "100%",
        boxSizing: "border-box",
      }}
    >
      <div
        className="comment-items-list"
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "8px",
          maxHeight: "320px",
          overflowY: "auto",
          paddingRight: "4px",
        }}
      >
        {comments.map((comment) => (
          <ElementCommentItem
            key={comment.id}
            comment={comment}
            onResolve={onResolveComment}
            onReanchor={onReanchorComment}
          />
        ))}
      </div>
      <NewCommentForm onAddComment={onAddComment} />
    </div>
  );
};
