import React, { Component, ErrorInfo, ReactNode } from "react";
import { useTranslation } from "../../hooks/useTranslation";
import { BrandIcon } from "./BrandIcon";

interface Props {
  children: ReactNode;
  fallbackTitle?: string;
  onReset?: () => void;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

const ErrorFallbackView: React.FC<{ fallbackTitle?: string; errorMessage?: string; onReset: () => void }> = ({
  fallbackTitle,
  errorMessage,
  onReset,
}) => {
  const { t } = useTranslation();
  return (
    <div
      role="alert"
      style={{
        display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center",
        height: "100%", width: "100%", padding: "24px", backgroundColor: "var(--bg-canvas)",
        color: "var(--text-primary)", textAlign: "center",
      }}
    >
      <div
        style={{
          padding: "24px", maxWidth: "500px", backgroundColor: "var(--bg-surface)",
          borderRadius: "var(--radius-md)", border: "1px solid var(--state-danger)", boxShadow: "var(--shadow-md)",
        }}
      >
        <div style={{ marginBottom: "12px", display: "flex", justifyContent: "center", color: "var(--state-warning)" }}>
          <BrandIcon name="warning" size={28} color="currentColor" />
        </div>
        <h3 style={{ margin: "0 0 8px 0", fontSize: "16px", fontWeight: 600 }}>
          {fallbackTitle || t("errors.renderFailedTitle", "Đã xảy ra lỗi hiển thị")}
        </h3>
        <p style={{ fontSize: "13px", color: "var(--text-secondary)", marginBottom: "16px", wordBreak: "break-word" }}>
          {errorMessage || t("errors.renderFailedBody", "Không thể vẽ đồ thị hoặc thành phần giao diện.")}
        </p>
        <button
          type="button"
          onClick={onReset}
          style={{
            padding: "8px 16px", fontSize: "13px", fontWeight: 500, color: "var(--action-primary-fg)",
            backgroundColor: "var(--action-primary)", border: "none", borderRadius: "var(--radius-xs)", cursor: "pointer",
          }}
        >
          {t("errors.recoverRetry", "Khôi phục & Thử lại")}
        </button>
      </div>
    </div>
  );
};

export class ErrorBoundary extends Component<Props, State> {
  public state: State = {
    hasError: false,
    error: null,
  };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("ErrorBoundary caught an unhandled error:", error, errorInfo);
  }

  private handleReset = () => {
    this.setState({ hasError: false, error: null });
    this.props.onReset?.();
  };

  public render() {
    if (this.state.hasError) {
      return (
        <ErrorFallbackView
          fallbackTitle={this.props.fallbackTitle}
          errorMessage={this.state.error?.message}
          onReset={this.handleReset}
        />
      );
    }

    return this.props.children;
  }
}
