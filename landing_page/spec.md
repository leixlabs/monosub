# MonoSub Landing Page - Design Specification

## Project Overview
**Product**: MonoSub - Local AI Subtitle Generation Desktop App
**Goal**: High-conversion landing page for Gumroad sales
**Target**: Content creators, video editors who need subtitle generation
**Priority**: Conversion Rate > Visual Flair

## Design Philosophy
- **Conversion-First Design**: Every element serves the goal of driving purchases
- **Clarity Over Creativity**: Clear value proposition, pain point highlighting
- **Trust Building**: Emphasize privacy, one-time purchase, lifetime value
- **No Blue/Purple**: Warm, distinctive color scheme to avoid generic SaaS look

---

## 1. Color System (STRICT - NO BLUE/PURPLE)

### Primary Palette
```
Background Dark:    #0a0a0f (Rich Black)
Card Background:    #141419 (Charcoal)
Border:             #2a2a35 (Slate Grey)

Accent Primary:     #f59e0b (Amber - Main CTA)
Accent Gradient:    #f59e0b → #ef4444 (Amber to Red-Orange)
Success:            #22c55e (Green - Checkmarks)

Text Primary:       #ffffff (White)
Text Secondary:     #9ca3af (Gray)
Text Muted:         #6b7280 (Dark Gray)
```

### Color Usage Rules
- **CTAs**: Amber (#f59e0b) with gradient on hover
- **Feature Highlights**: Amber accents
- **Success Indicators**: Green (#22c55e)
- **Backgrounds**: Dark (#0a0a0f) with subtle card layers
- **NO BLUE OR PURPLE ANYWHERE**

---

## 2. Typography

### Font Family
- **Primary**: Inter (Google Fonts)
- **Weights**: 400 (Regular), 600 (Semibold), 700 (Bold)

### Scale
```
Hero Headline:      5rem (80px) / Bold / Line-height: 1.1
Section Headline:   3rem (48px) / Semibold / Line-height: 1.2
Subsection:         2rem (32px) / Semibold / Line-height: 1.3
Body Large:         1.25rem (20px) / Regular / Line-height: 1.6
Body:               1rem (16px) / Regular / Line-height: 1.6
Button:             1rem (16px) / Semibold / Uppercase
Small:              0.875rem (14px) / Regular
```

---

## 3. Page Structure & Sections

### 3.1 Navigation Bar (Sticky)
- Logo: "MonoSub" text (no image)
- Navigation: Features, Pricing, FAQ
- CTA Button: "Get Lifetime Access" (Amber)
- Background: Semi-transparent dark with blur

### 3.2 Hero Section (Full Viewport)
**Goal**: Immediate value proposition + visual proof

**Layout**:
- Left: 50% - Text content
  - Headline: "Stop Paying Monthly for Subtitles. Own It Forever."
  - Subheadline: "100% Local AI Subtitle Generation. One Payment, Lifetime Access."
  - CTA: "Buy Now - Lifetime Access $XX" (Large amber button)
  - Trust Badge: "7-Day Money-Back Guarantee"
  
- Right: 50% - Product Screenshot
  - Image: `hero-mockup.png` (Existing product UI screenshot)
  - Float/Elevated: Subtle shadow, slight tilt

**Visual Asset Required**:
- Use existing: `/Users/lei/dev/personal/monosub/landing_page/imgs/hero-mockup.png`

### 3.3 Pain Point Section
**Goal**: Resonate with subscription fatigue

**Headline**: "Tired of Paying Every Month?"

**Layout**: Two-column comparison
- Left Column: "Cloud Subscription Services" (Problems)
  - ❌ $15-30/month forever
  - ❌ Upload videos to cloud (privacy risk)
  - ❌ Internet required
  - ❌ Usage limits

- Right Column: "MonoSub Solution" (Benefits)
  - ✅ Pay once, own forever
  - ✅ 100% local processing
  - ✅ Fully offline
  - ✅ Unlimited usage

**Visual Treatment**: Cards with red accents for problems, green checkmarks for solutions

### 3.4 Value Proposition Grid
**Headline**: "The Complete Subtitle Solution You Own"

**6 Feature Cards** (2 rows × 3 columns):
1. 🏠 **100% Local Processing**
   - No cloud, no uploads, complete privacy
   
2. 💰 **One-Time Purchase**
   - No subscriptions, no hidden fees
   
3. ⚡ **Hardware Accelerated**
   - Apple Silicon MPS / NVIDIA CUDA support
   
4. 🌍 **Multi-Language**
   - Auto-detect Chinese, English, and more
   
5. 📝 **Multiple Formats**
   - Export SRT, VTT, TXT
   
6. 🎯 **Smart Segmentation**
   - AI-powered sentence breaking

**Visual Assets**: SVG Icons (inline, no external images needed)

### 3.5 Pricing Comparison Table
**Headline**: "The Math is Simple"

**3-Column Table**:
| Timeframe | Cloud Subscription | MonoSub |
|-----------|-------------------|---------|
| 1 Month   | $19               | $XX     |
| 6 Months  | $114              | $XX     |
| 1 Year    | $228              | $XX     |
| 2 Years   | $456              | $XX     |

**Highlight**: "Save $XXX+ in the first year"

### 3.6 Technical Specs (For Technical Buyers)
**Headline**: "Powered by State-of-the-Art AI"

**Key Points**:
- FunASR / SenseVoice 2024 models
- Tauri 2.0 desktop framework
- Batch processing queue
- Currently: macOS (Apple Silicon optimized)
- Coming: Windows, Linux

**Visual**: Simple code-style presentation or tech stack icons

### 3.7 FAQ Section
**Headline**: "Common Questions"

**Accordion Items**:
1. **What platforms are supported?**
   - Currently macOS (M1/M2/M3). Windows & Linux coming soon.

2. **Is there a refund policy?**
   - Yes, 7-day money-back guarantee, no questions asked.

3. **Do I need internet to use it?**
   - No, 100% offline after installation.

4. **How does it compare to cloud services?**
   - More private, faster, and cheaper long-term. One-time purchase vs recurring fees.

5. **Will I get updates?**
   - Yes, lifetime updates included with your purchase.

6. **What video formats are supported?**
   - Most common formats: MP4, MOV, AVI, MKV, etc.

### 3.8 Final CTA Section
**Headline**: "Ready to Own Your Subtitle Workflow?"

**Layout**: Centered
- Subheadline: "Join creators who chose privacy and savings"
- CTA Button: "Get Lifetime Access - $XX" (Extra large)
- Trust Badges: 
  - "💳 Secure Payment via Gumroad"
  - "🔒 7-Day Money-Back Guarantee"
  - "✨ Instant Download"

### 3.9 Footer
- Copyright: "© 2025 MonoSub. Built by Matrix Agent."
- Links: Terms, Privacy, Contact
- Social: (Optional, if applicable)

---

## 4. Visual Assets Required

### Required Images
1. **Hero Product Screenshot** (Existing)
   - Path: `/Users/lei/dev/personal/monosub/landing_page/imgs/hero-mockup.png`
   - Usage: Hero section right column

### Optional Generated Images
For enhanced visual appeal (but not critical for conversion):
- Feature illustration: Conceptual image showing "local processing"
- Comparison visual: Cloud vs local workflow diagram

**Decision**: Use SVG icons primarily to keep page lightweight and fast-loading

---

## 5. Interaction & Animation

### Scroll Animations
- **Fade In**: Sections fade in when scrolling into view
- **Threshold**: 0.1 (trigger early for smoother experience)
- **Subtle**: No bouncing, sliding - just opacity transition

### Button Interactions
- **Hover**: Amber glow effect (`box-shadow` expansion)
- **Active**: Slight scale down (0.98)
- **Transition**: 200ms ease

### Navigation
- **Sticky**: Nav bar sticks to top with blur backdrop
- **Scroll Highlight**: Active section highlighted in nav

---

## 6. Technical Implementation

### Stack
- **HTML5**: Semantic structure
- **CSS**: Tailwind CSS 3.x (CDN)
- **JavaScript**: Vanilla JS for interactions
- **Fonts**: Google Fonts (Inter)
- **Icons**: Inline SVG (no external dependencies)

### Performance
- **Critical CSS**: Inline above-the-fold styles
- **Image Optimization**: Ensure hero-mockup.png is optimized (<500KB)
- **Lazy Loading**: Images below fold
- **No Heavy Dependencies**: Avoid jQuery, heavy animation libraries

### Responsive Breakpoints
```
Mobile:  < 640px  (Stack all columns)
Tablet:  640-1024px (2-column grids)
Desktop: > 1024px (Full layout)
```

---

## 7. Conversion Optimization Checklist

- [ ] Clear headline stating primary benefit
- [ ] Hero CTA above the fold
- [ ] Pain point section early (establish problem)
- [ ] Social proof / trust signals (money-back guarantee)
- [ ] Pricing comparison (justify value)
- [ ] Multiple CTAs (top, middle, bottom)
- [ ] FAQ to handle objections
- [ ] Fast load time (<2s)
- [ ] Mobile responsive
- [ ] High contrast CTA buttons (Amber on dark)

---

## 8. Copy Tone & Voice

### Tone
- **Confident but not arrogant**
- **Direct and benefit-focused**
- **Slightly rebellious** (challenging subscription model)
- **Technical credibility** (mention AI models, hardware acceleration)

### Language Mix
- **Primary**: English (broader reach)
- **Secondary**: Chinese (for key terms/phrases if target audience is Chinese-speaking creators)
- **Approach**: English-first, with optional Chinese translations for headlines

### Key Messaging
1. "Stop paying monthly"
2. "Your videos, your privacy"
3. "Pay once, own forever"
4. "Local AI, no cloud"

---

## 9. File Structure

```
/Users/lei/dev/personal/monosub/landing_page/
├── index.html          (Complete single-page HTML with inline CSS/JS)
├── imgs/
│   └── hero-mockup.png (Existing product screenshot)
└── spec.md            (This file)
```

---

## 10. Deployment

- **Method**: Use `deploy` tool (not local server)
- **Output**: Public URL for sharing
- **Validation**: Playwright test before deployment

---

## Design Reference Inspiration

**Style Influences** (NOT to copy, but for aesthetic reference):
- Stripe Landing Pages (clarity, simplicity)
- Linear (dark theme, subtle animations)
- Vercel (typography hierarchy, spacing)
- Superhuman (bold headlines, benefit-focused)

**Key Difference**: Warm amber accent instead of blue/purple tech colors

---

**End of Specification**
