# Syntax Highlighting Enhancements

## Overview

Enhanced difftastic's syntax highlighting system to provide richer, more colorful output for code diffs, particularly for TypeScript/TSX and JavaScript/JSX files. The previous implementation only supported 4 syntax categories (strings, comments, keywords/types, and normal text), making the output quite bland. This update expands support to 19 categories with a warm Zenburn-inspired color palette.

## Motivation

The original request was: *"can you make the syntax support more reach? it's very blend right now at least for ts/tsx"*

The previous highlighting was too basic, with most code elements appearing in the same color, making it harder to visually parse complex code structures.

## Changes Summary

### 1. Expanded AtomKind Enum
**File**: `src/parse/syntax.rs` (lines 615-656)

Added 14 new syntax category variants to the `AtomKind` enum:

- `Function` - Function names and method calls
- `Property` - Object properties and class fields
- `Number` - Numeric literals
- `Constant` - Constants (usually uppercase identifiers, boolean literals)
- `Variable` - Variables
- `Operator` - Operators (+, -, *, /, etc.)
- `Punctuation` - Punctuation (commas, semicolons)
- `Tag` - JSX/HTML tag names
- `Attribute` - JSX/HTML attribute names
- `Parameter` - Function parameters
- `Constructor` - Constructor names
- `Namespace` - Namespace/module names
- `Decorator` - Decorators (@decorator)
- `Label` - Labels

### 2. Enhanced Highlight Query Processing
**File**: `src/parse/tree_sitter_parser.rs` (lines 1230-1475)

Completely rewrote the `tree_highlights()` function to:
- Recognize 18 different tree-sitter capture types (expanded from 4)
- Map comprehensive tree-sitter capture names to AtomKind variants
- Support variations like `@function.call`, `@function.method`, `@property.field`, etc.

### 3. Updated Syntax Node Conversion
**File**: `src/parse/tree_sitter_parser.rs` (lines 1969-2023)

Modified `atom_from_cursor()` function to:
- Check all 18 highlight ID sets when converting tree-sitter nodes
- Properly categorize each syntax element based on its highlight type

### 4. Updated HighlightedNodeIds Structure
**File**: `src/parse/tree_sitter_parser.rs` (lines 1675-1695)

Expanded the `HighlightedNodeIds` struct with 14 new fields to store node IDs for all new highlight categories.

### 5. Added Rich Color Styling
**File**: `src/display/style.rs` (lines 447-920)

Implemented Zenburn-inspired color scheme for all syntax categories across four match kinds:
- `UnchangedToken` - Unchanged code with syntax highlighting
- `Novel` - New/modified code with syntax highlighting + diff colors
- `NovelWord` - Modified words with extra emphasis
- `UnchangedPartOfNovelItem` - Unchanged parts within modified items

### 6. Updated JSON Output Support
**File**: `src/display/json.rs`

Added all new highlight types to the `Highlight` enum for JSON serialization, ensuring API compatibility.

## Color Scheme

The implementation uses a Zenburn-inspired color palette with ANSI terminal colors:

| Syntax Category | Dark Background | Light Background |
|----------------|----------------|------------------|
| **Functions** | Bright Yellow | Yellow |
| **Properties** | Bright Cyan | Cyan |
| **Numbers** | Bright Yellow | Yellow |
| **Constants** | Bright Magenta | Magenta |
| **Tags** (JSX/HTML) | Bright Yellow | Yellow |
| **Attributes** | Bright Cyan | Cyan |
| **Parameters** | Cyan | Cyan |
| **Constructors** | Bright Yellow | Yellow |
| **Namespaces** | Bright Green | Green |
| **Decorators** | Yellow | Yellow |
| **Labels** | Bright Cyan | Cyan |
| **Strings** | Bright Magenta | Magenta |
| **Comments** | Bright Blue (italic) | Blue (italic) |
| **Keywords/Types** | Bold | Bold |
| **Variables** | Default | Default |
| **Operators** | Default | Default |
| **Punctuation** | Default | Default |

## Supported Languages

While implemented with TypeScript/TSX and JavaScript/JSX in mind, these enhancements benefit **any language** with tree-sitter highlight queries that use the standard capture names:

- JavaScript (.js)
- JavaScript + JSX (.jsx)
- TypeScript (.ts)
- TypeScript + JSX (.tsx)
- Python
- Rust
- Go
- Java
- C/C++
- And many more...

The improvements are automatic for any language where the tree-sitter grammar defines highlight queries using captures like `@function`, `@property`, `@number`, `@constant`, etc.

## Before and After

### Before
- **4 syntax categories**: strings (magenta), comments (blue italic), keywords/types (bold), normal
- Most code appeared in the default color
- Difficult to distinguish between functions, variables, properties, and numbers

### After
- **19 syntax categories** with distinct colors
- Functions, properties, and numbers stand out with bright colors
- JSX/HTML elements have clear visual distinction
- Constants and decorators are easily identifiable
- Much richer visual experience while maintaining readability

## Example Output

### TypeScript/TSX
```tsx
const Button: React.FC<ButtonProps> = ({ onClick, label, disabled = false }) => {
  const count = 100;          // count is yellow
  const message = "Hello";    // message default, string is magenta

  return (
    <button                   // button tag is yellow
      onClick={onClick}       // onClick attribute is cyan
      className="btn"         // className attribute is cyan
    >
      {label}
    </button>
  );
};
```

### JavaScript/JSX
- Function names like `Button`, `handleClick`, `log` → **Yellow**
- Properties/attributes like `onClick`, `className` → **Cyan**
- Numbers like `42`, `100` → **Yellow**
- Constants like `true`, `false` → **Magenta**
- Strings → **Magenta**
- Keywords → **Bold**

## Technical Notes

- All changes maintain backward compatibility
- No changes to the tree-sitter parsing logic itself
- Only highlight classification and color application were modified
- JSON output format includes all new highlight types
- No performance impact - same parsing process, just richer categorization

## Files Modified

1. `src/parse/syntax.rs` - AtomKind enum expansion
2. `src/parse/tree_sitter_parser.rs` - Highlight processing and node conversion
3. `src/display/style.rs` - Color styling implementation
4. `src/display/json.rs` - JSON serialization support

## Testing

Tested with:
- TypeScript/TSX files with interfaces, JSX elements, and various syntax constructs
- JavaScript/JSX files with functions, variables, and JSX
- Build process: Successfully compiled with no errors
- Output: Verified rich syntax highlighting in terminal with ANSI colors

## Future Enhancements

Potential areas for future improvement:
- Add more specific highlighting for type annotations
- Support for additional tree-sitter captures as they're added to grammars
- User-configurable color schemes
- Support for 24-bit RGB colors for even more nuanced highlighting
