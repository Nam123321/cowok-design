# Modular Type Scale & Multilingual Typography Reference (`typography`)

## 1. Baseline Type Tokens (Adaptable per Brand Tone)
```css
:root {
  --font-display: "Plus Jakarta Sans", "Inter", system-ui, sans-serif;
  --font-body: "Inter", system-ui, -apple-system, sans-serif;
  --font-mono: "JetBrains Mono", ui-monospace, SFMono-Regular, monospace;
  --text-xs: 0.75rem;    /* 12px · captions / badges */
  --text-sm: 0.875rem;   /* 14px · compact UI / table rows */
  --text-base: 1rem;     /* 16px · body copy */
  --text-lg: 1.25rem;    /* 20px · section headings */
  --text-xl: 1.563rem;   /* 25px · screen titles */
  --text-2xl: 1.953rem;  /* 31px · hero / slide action title */
}
.metric-value {
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
  line-height: 1.15;
}
```

## 2. Multilingual Script Tuning (Minimum Floor)
- **Vietnamese Stacked Diacritics:** Never set `line-height < 1.35` on headings or `< 1.5` on body paragraphs; verify stacked tone marks (`ễ`, `ử`, `ậ`) render cleanly with zero top clipping inside badges and buttons.
- **CJK Scripts:** Use slightly looser line-height (`1.6–1.75`), disable forced `text-transform: uppercase`, and respect natural word-break rules (`word-break: keep-all` where appropriate).
- **RTL Layouts:** Pair typography with logical CSS properties (`margin-inline-start`, `padding-inline-end`, `text-align: start`) so Arabic/Hebrew flows mirror cleanly without broken alignment.
