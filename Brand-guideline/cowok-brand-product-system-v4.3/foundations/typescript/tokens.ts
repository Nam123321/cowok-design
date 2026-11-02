export const cowokThemeSystem = {
  "meta": {
    "brand": "cowok.ai",
    "version": "1.1.0",
    "principle": "Warm in light. Calm in dark. Quiet by default. Color only when meaningful.",
    "themes": [
      "warmLight",
      "focusLight",
      "night"
    ],
    "lowChromaFocus": "night only"
  },
  "brand": {
    "blue": "#0057FF",
    "cyan": "#00D4FF",
    "orange": "#FF8A00",
    "navy": "#081E3A",
    "gray": "#667085",
    "light": "#F7FAFC",
    "white": "#FFFFFF"
  },
  "themes": {
    "warmLight": {
      "mode": "light",
      "lowChromaFocus": false,
      "bg": {
        "canvas": "#F7F5F0",
        "workspace": "#FFFDFC",
        "surface": "#FFFFFF",
        "surfaceSubtle": "#F2F0EB",
        "surfaceRaised": "#FFFFFF",
        "surfaceHover": "#F5F7FA",
        "surfaceSelected": "#EDF4FF"
      },
      "text": {
        "primary": "#081E3A",
        "secondary": "#344054",
        "muted": "#667085",
        "disabled": "#98A2B3",
        "onAction": "#FFFFFF",
        "onStrong": "#FFFFFF"
      },
      "border": {
        "default": "#E6E2DB",
        "strong": "#D6D1C8",
        "focus": "#90AEFF"
      },
      "human": {
        "accent": "#4B7FFF",
        "soft": "#EDF3FF"
      },
      "agent": {
        "accent": "#E89A35",
        "soft": "#FFF3E3"
      }
    },
    "focusLight": {
      "mode": "dim-light",
      "lowChromaFocus": false,
      "bg": {
        "canvas": "#ECEAE4",
        "workspace": "#F4F2ED",
        "surface": "#FAF9F6",
        "surfaceSubtle": "#E5E2DB",
        "surfaceRaised": "#FAF9F6",
        "surfaceHover": "#EEECE7",
        "surfaceSelected": "#E7EEF9"
      },
      "text": {
        "primary": "#182230",
        "secondary": "#475467",
        "muted": "#667085",
        "disabled": "#98A2B3",
        "onAction": "#FFFFFF",
        "onStrong": "#FFFFFF"
      },
      "border": {
        "default": "#D8D4CC",
        "strong": "#C8C2B8",
        "focus": "#8099D8"
      },
      "human": {
        "accent": "#5579C9",
        "soft": "#E9EEF8"
      },
      "agent": {
        "accent": "#B9772B",
        "soft": "#F5EEE5"
      }
    },
    "night": {
      "mode": "dark",
      "lowChromaFocus": true,
      "bg": {
        "canvas": "#0B1220",
        "workspace": "#111827",
        "surface": "#172033",
        "surfaceSubtle": "#1C2940",
        "surfaceRaised": "#22314B",
        "surfaceHover": "#17263D",
        "surfaceSelected": "#16294A"
      },
      "text": {
        "primary": "#F1F5F9",
        "secondary": "#CBD5E1",
        "muted": "#94A3B8",
        "disabled": "#64748B",
        "onAction": "#081E3A",
        "onStrong": "#081E3A"
      },
      "border": {
        "default": "#26354D",
        "strong": "#344761",
        "focus": "#6C8FD9"
      },
      "human": {
        "accent": "#4D86FF",
        "soft": "#152A4C"
      },
      "agent": {
        "accent": "#FF9A2F",
        "soft": "#211B16"
      },
      "lowChroma": {
        "humanAccent": "#65789E",
        "agentAccent": "#8F765E",
        "success": "#72877A",
        "warning": "#8B7D68",
        "danger": "#8B7070",
        "humanSoft": "surface 88% + muted blue 12%",
        "agentSoft": "surface 92% + brand orange 8%"
      }
    }
  },
  "actions": {
    "warmLight": {
      "primary": "#081E3A",
      "primaryHover": "#102A50",
      "link": "#0057FF"
    },
    "focusLight": {
      "primary": "#182230",
      "primaryHover": "#253248",
      "link": "#315FB6"
    },
    "night": {
      "primary": "#F1F5F9",
      "primaryHover": "#FFFFFF",
      "link": "#6F9BFF"
    }
  },
  "semanticStates": {
    "success": "#3E8A5D",
    "warning": "#A87424",
    "error": "#B45555",
    "info": "#4B7FFF"
  },
  "abstraction": {
    "colorRoles": {
      "bg.canvas": "Global app background. Never used for cards.",
      "bg.workspace": "Primary working area behind content modules.",
      "bg.surface": "Default card, table, dialog, input container.",
      "bg.surfaceSubtle": "Secondary grouping surface such as Kanban column, sidebar group, table header.",
      "bg.surfaceRaised": "Popover, modal, dropdown, floating panel.",
      "bg.surfaceHover": "Hover state only.",
      "bg.surfaceSelected": "Selected item/row/tab background.",
      "text.primary": "Headings, important labels, main body copy.",
      "text.secondary": "Supporting copy and ordinary labels.",
      "text.muted": "Metadata, timestamps, helper copy.",
      "text.disabled": "Disabled/inactive text only.",
      "text.onAction": "Text/icons placed on primary action fill.",
      "border.default": "Default dividers and component outlines.",
      "border.strong": "Strong hierarchy boundaries only.",
      "border.focus": "Keyboard/input focus ring.",
      "human.accent": "Human-owned active/action state.",
      "human.soft": "Human-associated low-emphasis background.",
      "agent.accent": "Agent-owned active/action state.",
      "agent.soft": "Agent-associated subtle background, never a full page/card field."
    }
  },
  "components": {
    "button": {
      "primary": {
        "bg": "action.primary",
        "text": "text.onAction",
        "border": "transparent",
        "hover": "action.primaryHover",
        "usage": "Main explicit user action only."
      },
      "secondary": {
        "bg": "bg.surface",
        "text": "text.primary",
        "border": "border.default",
        "hover": "bg.surfaceHover"
      },
      "ghost": {
        "bg": "transparent",
        "text": "text.secondary",
        "border": "transparent",
        "hover": "bg.surfaceHover"
      },
      "humanAction": {
        "bg": "human.accent",
        "text": "#FFFFFF",
        "usage": "Only when Human ownership/action is semantically important."
      },
      "agentAction": {
        "bg": "agent.accent",
        "text": "night uses #081E3A; light modes use high-contrast dark/white as validated",
        "usage": "Agent execution/review action only. Not a general CTA color."
      }
    },
    "input": {
      "bg": "bg.surface",
      "text": "text.primary",
      "placeholder": "text.muted",
      "border": "border.default",
      "focusBorder": "border.focus",
      "focusRing": "border.focus at low opacity",
      "error": "semantic.error"
    },
    "card": {
      "default": {
        "bg": "bg.surface",
        "border": "border.default",
        "text": "text.primary"
      },
      "subtle": {
        "bg": "bg.surfaceSubtle",
        "border": "border.default"
      },
      "raised": {
        "bg": "bg.surfaceRaised",
        "border": "border.default",
        "shadow": "elevation.overlay"
      }
    },
    "reviewRequired": {
      "bg": "bg.surfaceSubtle",
      "border": "border.default",
      "accent": "agent.accent as 3px edge/icon only",
      "action": "agentAction",
      "rule": "Never fill the full review container with orange/amber."
    },
    "selection": {
      "bg": "bg.surfaceSelected",
      "text": "text.primary",
      "accent": "human.accent unless domain semantics specify otherwise"
    },
    "status": {
      "rule": "Status never uses color alone; always label + optional icon.",
      "success": "semantic.success",
      "warning": "semantic.warning",
      "error": "semantic.error",
      "info": "semantic.info"
    }
  }
} as const;
