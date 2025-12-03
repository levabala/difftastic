# 24-bit RGB Color Support for Background Diff Colors

## Overview

This feature adds full 24-bit truecolor support to difftastic's `--background-diff-colors` mode, allowing users to specify custom RGB colors for added and removed content backgrounds.

## Commit Information

- **Commit:** `a6f0bee5f`
- **Files Changed:** 4 files (+127 insertions, -14 deletions)
- **Modified Files:**
  - `src/display/inline.rs`
  - `src/display/side_by_side.rs`
  - `src/display/style.rs`
  - `src/options.rs`

## Implementation Details

### 1. RgbColor Type (`src/display/style.rs:33-63`)

Added a new type to represent 24-bit RGB colors:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RgbColor {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
}
```

Features:
- Parse hex color strings like `"#336699"` or `"336699"`
- Validates input format (must be 6 hex digits)
- Provides helpful error messages for invalid input

### 2. DisplayOptions Extension (`src/options.rs:51-54`)

Added two new optional fields:

```rust
pub(crate) struct DisplayOptions {
    // ... existing fields ...
    pub(crate) diff_color_added_bg: Option<RgbColor>,
    pub(crate) diff_color_removed_bg: Option<RgbColor>,
}
```

Defaults to `None` for backward compatibility.

### 3. CLI Flags (`src/options.rs:273-285`)

Added two new command-line flags:

- `--diff-color-added-bg <HEX>` - Custom background color for added content
- `--diff-color-removed-bg <HEX>` - Custom background color for removed content

### 4. Environment Variables

Supports configuration via environment variables:
- `DFT_DIFF_COLOR_ADDED_BG`
- `DFT_DIFF_COLOR_REMOVED_BG`

### 5. Color Rendering (`src/display/style.rs:345-389`)

Updated `novel_style()` function to:
- Accept optional `rgb_added` and `rgb_removed` parameters
- Use `style.on_truecolor(r, g, b)` from owo-colors when RGB colors provided
- Fall back to ANSI colors when RGB colors are `None`

## Usage Examples

### Basic Usage

```bash
# Use custom 24-bit RGB colors with hex values
difft --background-diff-colors on \
      --diff-color-added-bg 003366 \
      --diff-color-removed-bg 663300 \
      old.js new.js
```

### With # Prefix

```bash
difft --background-diff-colors on \
      --diff-color-added-bg '#336699' \
      --diff-color-removed-bg '#996633' \
      old.py new.py
```

### Using Environment Variables

```bash
export DFT_BACKGROUND_DIFF_COLORS=on
export DFT_DIFF_COLOR_ADDED_BG="#004488"
export DFT_DIFF_COLOR_REMOVED_BG="#884400"

difft old.rs new.rs
```

### Subtle Colors for Dark Terminals

```bash
# Very subtle (barely visible on black backgrounds)
export DFT_DIFF_COLOR_ADDED_BG="#002300"    # Green: RGB(0, 35, 0)
export DFT_DIFF_COLOR_REMOVED_BG="#230000"  # Red: RGB(35, 0, 0)

# More visible
export DFT_DIFF_COLOR_ADDED_BG="#003800"    # Green: RGB(0, 56, 0)
export DFT_DIFF_COLOR_REMOVED_BG="#380000"  # Red: RGB(56, 0, 0)
```

## Color Value Guidelines

### Hex to Decimal Conversion

- `#00` = 0 (completely off)
- `#33` = 51 (~20% brightness)
- `#66` = 102 (~40% brightness)
- `#99` = 153 (~60% brightness)
- `#CC` = 204 (~80% brightness)
- `#FF` = 255 (100% brightness)

### Recommended Values for Dark Terminals

For good visibility on black backgrounds, use values of 35+ (0x23+):

- **Very Subtle:** `#002300` to `#003000` (35-48)
- **Subtle:** `#003000` to `#004000` (48-64)
- **Moderate:** `#004000` to `#006000` (64-96)
- **Visible:** `#006000` to `#008800` (96-136)

### Traditional Diff Colors

```bash
# GitHub-style (brighter)
export DFT_DIFF_COLOR_ADDED_BG="#e6ffed"    # Light green
export DFT_DIFF_COLOR_REMOVED_BG="#ffeef0"  # Light red

# Darker green/red (for dark terminals)
export DFT_DIFF_COLOR_ADDED_BG="#003300"
export DFT_DIFF_COLOR_REMOVED_BG="#330000"
```

## Technical Details

### ANSI Truecolor Escape Codes

The implementation uses the ANSI 24-bit truecolor format:
```
ESC[48;2;R;G;Bm  (set background color)
ESC[49m          (reset background)
```

Example: `\033[48;2;0;56;0m` sets background to RGB(0, 56, 0)

### Terminal Compatibility

Requires a terminal that supports 24-bit color (truecolor):
- ✅ Alacritty
- ✅ iTerm2
- ✅ Kitty
- ✅ WezTerm
- ✅ Windows Terminal
- ✅ GNOME Terminal (recent versions)
- ⚠️ Terminal.app (macOS) - limited support
- ❌ Many older terminals

### Testing Truecolor Support

A bash script to test terminal truecolor support is available at:
`/tmp/test_truecolor.sh` (can be recreated if needed)

## Backward Compatibility

- Default behavior unchanged when flags are not specified
- Falls back to ANSI colors (`on_bright_black()` or `on_black()`) when RGB colors are `None`
- Only applies when `--background-diff-colors on` is enabled

## Error Handling

Invalid hex colors produce helpful error messages:

```bash
$ difft --background-diff-colors on --diff-color-added-bg "invalid" old.rs new.rs
error: Invalid hex color 'invalid': expected 6 characters (RRGGBB)
```

## Future Enhancements

Possible future additions:
- Support for HSL/HSV color formats
- Named color presets (e.g., `--diff-colors github`, `--diff-colors solarized`)
- Automatic color adjustment based on terminal background detection
- Per-file-type color customization

## References

- ANSI Truecolor: https://gist.github.com/XVilka/8346728
- owo-colors documentation: https://docs.rs/owo-colors/
- Original background diff colors feature: commit `043b8d32a`
