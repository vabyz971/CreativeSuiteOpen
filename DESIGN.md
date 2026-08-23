---
name: CreativeSuiteOpen (Rust/Iced Edition)
framework: Iced 0.14
language: Rust
colors:
  surface: '#131313'
  surface-dim: '#131313'
  surface-bright: '#393939'
  surface-container-lowest: '#0e0e0e'
  surface-container-low: '#1c1b1b'
  surface-container: '#201f1f'
  surface-container-high: '#2a2a2a'
  surface-container-highest: '#353534'
  on-surface: '#e5e2e1'
  on-surface-variant: '#c1c6d7'
  inverse-surface: '#e5e2e1'
  inverse-on-surface: '#313030'
  outline: '#8b90a0'
  outline-variant: '#414755'
  surface-tint: '#adc6ff'
  primary: '#adc6ff'
  on-primary: '#002e69'
  primary-container: '#4b8eff'
  on-primary-container: '#00285c'
  inverse-primary: '#005bc1'
  secondary: '#c8c6c6'
  on-secondary: '#303030'
  secondary-container: '#474747'
  on-secondary-container: '#b6b5b4'
  tertiary: '#ffb595'
  on-tertiary: '#571e00'
  tertiary-container: '#ef6719'
  on-tertiary-container: '#4c1a00'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#d8e2ff'
  primary-fixed-dim: '#adc6ff'
  on-primary-fixed: '#001a41'
  on-primary-fixed-variant: '#004493'
  secondary-fixed: '#e4e2e1'
  secondary-fixed-dim: '#c8c6c6'
  on-secondary-fixed: '#1b1c1c'
  on-secondary-fixed-variant: '#474747'
  tertiary-fixed: '#ffdbcc'
  tertiary-fixed-dim: '#ffb595'
  on-tertiary-fixed: '#351000'
  on-tertiary-fixed-variant: '#7c2e00'
  background: '#131313'
  on-background: '#e5e2e1'
  surface-variant: '#353534'
  text-primary: '#FFFFFF'
  text-secondary: '#A0A0A0'
  border-subtle: '#2D2D2D'
typography:
  headline-lg:
    fontFamily: Hanken Grotesk
    fontSize: 24px
    fontWeight: '700'
    lineHeight: 32px
  headline-md:
    fontFamily: Hanken Grotesk
    fontSize: 18px
    fontWeight: '600'
    lineHeight: 24px
  body-md:
    fontFamily: Hanken Grotesk
    fontSize: 14px
    fontWeight: '400'
    lineHeight: 20px
  label-sm:
    fontFamily: Hanken Grotesk
    fontSize: 11px
    fontWeight: '400'
    lineHeight: 16px
rounded:
  sm: 0.125rem
  DEFAULT: 0.25rem
  md: 0.375rem
  lg: 0.5rem
  xl: 0.75rem
  full: 9999px
spacing:
  margin-page: 24px
  gutter-panel: 16px
  stack-sm: 8px
  stack-md: 12px
---

# Design System: Lumina Creative

## Color Palette (RGBA/Hex for Iced)
Iced uses `iced::Color` or hex strings. These tokens are optimized for the `Theme::Custom` implementation in Iced 0.14.

| Token | Hex | Iced Color Literal | Usage |
|-------|-----|--------------------|-------|
| `SURFACE` | `#131313` | `Color::from_rgb8(19, 19, 19)` | Main application background |
| `SURFACE_BRIGHT` | `#393939` | `Color::from_rgb8(57, 57, 57)` | Floating panel backgrounds |
| `ACCENT` | `#007AFF` | `Color::from_rgb8(0, 122, 255)` | Primary actions, sliders, and highlights |
| `TEXT_PRIMARY` | `#FFFFFF` | `Color::from_rgb8(255, 255, 255)` | Headings and high-contrast text |
| `TEXT_SECONDARY` | `#A0A0A0` | `Color::from_rgb8(160, 160, 160)` | Labels and inactive states |
| `BORDER` | `#2D2D2D` | `Color::from_rgb8(45, 45, 45)` | Subtle separators and container outlines |

## Typography
Iced 0.14 utilizes system fonts or loaded TTF/OTF files via `iced::Font`.

- **Primary Font**: Hanken Grotesk (Sans-serif)
- **Size Scale**:
    - `H1`: 24pt (Bold)
    - `H2`: 18pt (Semi-bold)
    - `Body`: 14pt (Regular)
    - `Caption`: 11pt (Regular)

## Component Styles (Iced `StyleSheet`)

### Containers
- **Floating Panel**:
    - `background`: `SURFACE_BRIGHT` with 80% opacity for glassmorphism effect.
    - `border_radius`: `4.0` (Round Four).
    - `border_width`: `1.0`.
    - `border_color`: `BORDER`.

### Buttons
- **Primary**:
    - `background`: `ACCENT`.
    - `text_color`: `TEXT_PRIMARY`.
    - `border_radius`: `6.0`.
- **Secondary/Icon**:
    - `background`: `Transparent`.
    - `text_color`: `TEXT_SECONDARY`.
    - `hovered`: Background `#FFFFFF` (10% opacity).

### Inputs (Sliders & Knobs)
- **Slider Rail**: `#2D2D2D`.
- **Slider Handle**: `ACCENT`.
- **Knob Accent**: `ACCENT` (used in Synth/Mixer views).

## Iced 0.14 Implementation Note
To implement this in Rust:
```rust
use iced::{Color, theme, Theme};

// Example of a custom palette for Iced
let palette = theme::Palette {
    background: Color::from_rgb8(19, 19, 19),
    text: Color::WHITE,
    primary: Color::from_rgb8(0, 122, 255),
    success: Color::from_rgb8(40, 167, 69),
    danger: Color::from_rgb8(220, 53, 69),
};

// Define custom styling for the "Floating" effect
pub fn floating_panel(theme: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(Color { a: 0.8, ..Color::from_rgb8(57, 57, 57) }.into()),
        border_radius: 4.0.into(),
        border_width: 1.0,
        border_color: Color::from_rgb8(45, 45, 45),
        ..Default::default()
    }
}
```

## Icons
Use `iced_aw` or `iced_fonts` (Lucide/Bootstrap icons) mapped to the `TEXT_SECONDARY` color for consistent toolbars.