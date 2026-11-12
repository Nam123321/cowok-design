# Craft Rule: Spacing, Grids & Concentric Radii

## Principles
1. **8px Baseline Grid**: Align component dimensions and margins to 4px/8px multiples (4, 8, 12, 16, 24, 32, 48, 64px).
2. **Concentric Border Radius**: When nesting rounded containers, inner radius must be concentric:
   $$R_{\text{inner}} = \max(4, R_{\text{outer}} - P/2)$$
   where $P$ is the padding between outer and inner containers.
3. **Internal vs External Padding**: Container padding $P_{\text{outer}}$ must be greater than or equal to internal child item padding $P_{\text{inner}}$.
4. **Button Padding Balance**: Horizontal button padding must be at least 1.25x vertical padding (e.g. 8px 16px or 10px 20px).
