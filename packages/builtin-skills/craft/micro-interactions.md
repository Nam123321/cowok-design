# Craft Rule: Micro-Interactions & Motion Budget

## Principles
1. **Duration Bounds**: UI micro-interactions must resolve between 150ms and 250ms. Never exceed 300ms for utility interactions.
2. **Motion Has a Cause**: "Every motion must have an origin and destination." Animate scale, opacity, and transform; never animate reflow properties (`width`, `height`, `top`, `left`).
3. **Easing Curves**: Use `cubic-bezier(0.16, 1, 0.3, 1)` (ease-out-expo) for entrances, and `cubic-bezier(0.7, 0, 0.84, 0)` for dismissals.
4. **Touch Targets**: Minimum interactive touch target is 44x44px.
