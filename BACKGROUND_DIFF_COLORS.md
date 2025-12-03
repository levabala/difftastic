# Background Diff Colors Feature

## Overview
Added a new `--background-diff-colors` flag that enables background highlighting for diff content while preserving syntax highlighting in the foreground.

## Usage

```bash
# Enable background diff colors
difftastic --background-diff-colors on old.js new.js

# Or using environment variable
export DFT_BACKGROUND_DIFF_COLORS=on
difftastic old.js new.js

# Combined with other flags
difftastic --syntax-highlight on --background-diff-colors on --background dark old.py new.py
```

## Behavior

### Default Mode (`--background-diff-colors off`)
- Added/removed content uses **foreground** red/green colors
- Syntax highlighting is overridden by diff colors
- Original difftastic behavior

### Background Mode (`--background-diff-colors on`)
- Added/removed content uses **subtle gray background**
- Syntax highlighting colors (magenta strings, blue comments, etc.) remain visible in the foreground
- Much less visually aggressive
- Works perfectly in side-by-side view where position indicates add/remove

## Color Choices

### Dark Terminals (`--background dark`, default)
- Background: `on_bright_black()` - subtle gray
- Preserves all syntax highlighting foreground colors

### Light Terminals (`--background light`)
- Background: `on_black()` - darker gray
- Preserves all syntax highlighting foreground colors

## Implementation Details

### Files Modified
1. **src/options.rs**
   - Added `background_diff_colors: bool` field to `DisplayOptions`
   - Added CLI argument `--background-diff-colors` with values `on`/`off`
   - Added environment variable support: `DFT_BACKGROUND_DIFF_COLORS`

2. **src/display/style.rs**
   - Updated `novel_style()` function to accept `use_background: bool` parameter
   - When enabled, applies gray background colors instead of red/green foreground
   - Updated `color_positions()` to apply syntax highlighting to Novel content when background mode is on
   - Updated `apply_colors()` signature to pass through the flag

3. **src/display/side_by_side.rs**
   - Updated all `apply_colors()` and `color_positions()` call sites
   - Updated `highlight_positions()` function signature
   - Line numbers always use foreground colors regardless of flag

4. **src/display/inline.rs**
   - Updated all `apply_colors()` call sites to pass the new parameter

## Design Rationale

### Why Gray Instead of Red/Green?
- **Less aggressive** - neutral gray is much easier on the eyes
- **Better syntax visibility** - colored syntax stands out more against neutral background
- **Side-by-side works perfectly** - left/right position already indicates removed vs added
- **Reduced cognitive load** - color isn't fighting for attention with the actual code

### Why Preserve Syntax Highlighting?
- Users who enable this feature want **both** diff context and syntax information
- Syntax colors help identify tokens (strings, keywords, comments) at a glance
- More information density without visual clutter

## Technical Notes

- Uses standard ANSI colors (`on_bright_black()`, `on_black()`) for universal terminal compatibility
- No 24-bit color dependency - works in all terminals including Alacritty
- Line numbers always use foreground colors (red/green) to maintain visual hierarchy
- Single-column display (file additions/removals) uses foreground colors for clarity

## Full-Line Background Highlighting

### Two Additional Flags

**Flag 1: `--full-line-background on/off`**
Environment variable: `DFT_FULL_LINE_BACKGROUND`

Extends background color to fill the entire line width for complete line additions/removals (lines that are entirely novel, not partial/word-level changes).

- Only affects lines that contain exclusively novel content
- Background color fills to terminal width
- Requires `--background-diff-colors on` to be enabled
- Default: `off`

**Flag 2: `--background-include-whitespace on/off`**
Environment variable: `DFT_BACKGROUND_INCLUDE_WHITESPACE`

Includes trailing spaces between consecutive changed lines in the background highlight, creating continuous visual blocks.

- Applies to all lines with novel content (including partial changes)
- Creates solid colored blocks for consecutive changed lines with no gaps
- Requires `--background-diff-colors on` to be enabled
- Default: `off`

### Usage Examples

```bash
# Full-line backgrounds only (extends complete line changes to full width)
difftastic --background-diff-colors on --full-line-background on old.js new.js

# Continuous blocks for any changed lines (no gaps between consecutive lines)
difftastic --background-diff-colors on --background-include-whitespace on old.js new.js

# Both together for maximum visual continuity
difftastic --background-diff-colors on --full-line-background on --background-include-whitespace on old.js new.js

# Using environment variables
export DFT_BACKGROUND_DIFF_COLORS=on
export DFT_FULL_LINE_BACKGROUND=on
export DFT_BACKGROUND_INCLUDE_WHITESPACE=on
difftastic old.js new.js
```

### When to Use

- **`--full-line-background`**: Use when you want complete line changes (not partial edits) to be more visually prominent by extending the background to the full terminal width. Best for line-level diffs.

- **`--background-include-whitespace`**: Use when you want consecutive changed lines to appear as solid colored blocks without visual gaps. Creates better visual grouping of related changes.

- **Both flags together**: Provides the strongest visual distinction for complete line changes while maintaining continuous blocks across consecutive modifications.

### Flag Independence

Both flags are independent and can be used separately or together:
- `full_line_background` only applies padding to lines that are entirely novel
- `background_include_whitespace` applies padding to all lines with novel content (full or partial)
- When both are enabled, `full_line_background` takes precedence for complete line changes
