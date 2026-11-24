# Adaptive Slide Framing & Miniature Stage Patterns (`stageshow-slide`)

## 1. Adaptive Viewport Ratios (No Rigid Pixel Ceiling)
| Ratio Mode | CSS Framing | Best Use Case |
| :--- | :--- | :--- |
| **`16:9`** | `aspect-ratio: 16 / 9; container-type: inline-size;` | Standard widescreen decks, conference monitors, video export |
| **`16:10`** | `aspect-ratio: 16 / 10; container-type: inline-size;` | Laptop-native executive reviews and dense data tables |
| **`Auto`** | Fluid viewport height (`min-height: 100dvh; max-width: 1440px`) | Web-native scrollytelling briefings and responsive mobile/tablet viewing |

## 2. Compositional Patterns
- **Asymmetric Visual-First Split (`60/40`):** Place the primary chart, SVG topology, or miniature UI `<Window>`/`<Phone>` frame in the 60% hero zone and concise narrative proof in the 40% rail.
- **Breathing Room Floor:** Maintain `>= 30%` open canvas whitespace and `>= 24px` outer safe margin so projected slides never crowd bezel edges.
- **Causal Walkthrough Beats:** Pair step-through highlights (`data-step="1..N"`) with keyboard `←`/`→` navigation so presenters can reveal baseline → complication → resolution smoothly.
- **Semantic Tokens Only:** Inherit slide surfaces, typography, and accent hues from the active project `tokens.css` so dark/light/custom themes switch cleanly without contrast regressions.
