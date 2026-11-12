import React, { useState, useEffect } from "react";
import { ClarificationVariant } from "../../../lib/validations/sdi";
import { ClarificationCard } from "./ClarificationCard";

interface VisualClarificationModalProps {
  isOpen: boolean;
  variants: ClarificationVariant[];
  onConfirm: (selectedVariantId: string) => void;
  onClose: () => void;
}

const ModalHeader: React.FC = () => (
  <div>
    <h2 className="text-base font-bold text-foreground">Làm Rõ Ý Định Thiết Kế (Visual Clarification)</h2>
    <p className="text-xs text-muted-foreground mt-1">
      Yêu cầu có độ mơ hồ cao (&ge; 0.45). Vui lòng chọn 1 phương án định hướng trước khi sinh mã.
    </p>
  </div>
);

interface ModalFooterProps {
  selectedId: string | null;
  onCancel: () => void;
  onConfirm: () => void;
}

const ModalFooter: React.FC<ModalFooterProps> = ({ selectedId, onCancel, onConfirm }) => (
  <div className="flex justify-end gap-2 pt-2 border-t border-border">
    <button type="button" onClick={onCancel} className="px-3 py-1.5 text-xs rounded-md border border-border">
      Hủy Bỏ
    </button>
    <button
      type="button"
      disabled={!selectedId}
      onClick={onConfirm}
      className="px-3 py-1.5 text-xs font-semibold rounded-md bg-primary text-primary-foreground disabled:opacity-50"
    >
      Xác Nhận &amp; Tiếp Tục
    </button>
  </div>
);

export const VisualClarificationModal: React.FC<VisualClarificationModalProps> = ({
  isOpen,
  variants,
  onConfirm,
  onClose,
}) => {
  const [selectedId, setSelectedId] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) setSelectedId(null);
  }, [isOpen, variants]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div className="w-full max-w-2xl bg-card border border-border rounded-xl p-5 shadow-2xl flex flex-col gap-4">
        <ModalHeader />
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          {variants.map((v) => (
            <ClarificationCard
              key={v.id}
              variant={v}
              isSelected={selectedId === v.id}
              onSelect={setSelectedId}
            />
          ))}
        </div>
        <ModalFooter
          selectedId={selectedId}
          onCancel={() => {
            setSelectedId(null);
            onClose();
          }}
          onConfirm={() => selectedId && onConfirm(selectedId)}
        />
      </div>
    </div>
  );
};
