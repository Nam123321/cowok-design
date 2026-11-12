import React, { Component, ErrorInfo, ReactNode } from "react";

interface Props {
  children: ReactNode;
  fallbackTitle?: string;
  onReset?: () => void;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

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
        <div
          role="alert"
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            height: "100%",
            width: "100%",
            padding: "24px",
            backgroundColor: "var(--bg-canvas)",
            color: "var(--text-primary)",
            textAlign: "center",
          }}
        >
          <div
            style={{
              padding: "24px",
              maxWidth: "500px",
              backgroundColor: "var(--bg-surface)",
              borderRadius: "8px",
              border: "1px solid var(--state-danger)",
              boxShadow: "var(--shadow-md)",
            }}
          >
            <div style={{ fontSize: "28px", marginBottom: "12px" }}>⚠️</div>
            <h3 style={{ margin: "0 0 8px 0", fontSize: "16px", fontWeight: 600 }}>
              {this.props.fallbackTitle || "Đã xảy ra lỗi hiển thị"}
            </h3>
            <p
              style={{
                fontSize: "13px",
                color: "var(--text-secondary)",
                marginBottom: "16px",
                wordBreak: "break-word",
              }}
            >
              {this.state.error?.message || "Không thể vẽ đồ thị hoặc thành phần giao diện."}
            </p>
            <button
              type="button"
              onClick={this.handleReset}
              style={{
                padding: "8px 16px",
                fontSize: "13px",
                fontWeight: 500,
                color: "var(--action-primary-fg)",
                backgroundColor: "var(--action-primary)",
                border: "none",
                borderRadius: "6px",
                cursor: "pointer",
              }}
            >
              Khôi phục & Thử lại
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
