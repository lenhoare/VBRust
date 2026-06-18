# VBR (Visual Basic to Rust) Transpiler

A transpiler that converts Visual Basic code to Rust-based VBR syntax, designed to help VB programmers transition to Rust.

## Philosophy

- **VBA syntax, Rust semantics** - Never choose the VB way when there's a conflict
- **Idiomatic Rust output** - Generate proper Rust code
- **Educational** - Warn about pitfalls but don't preach
- **Inline Rust escape hatch** - Allow advanced features through Rust code

## Features

### Supported Language Features

#### Data Types
- Integer → i32 (copies freely - fixed size)
- Long → i32 (copies freely - fixed size)
- LongLong → i64
- Single → f32 (copies freely - fixed size)
- Double → f64 (copies freely - fixed size)
- Boolean → bool
- Byte → u8
- String → String / &str
- User-defined types
- HashMap<K, V>
- Vec<T>
- Result<T, E>

#### Variable Declarations
```vb
Dim a As Long = 5
Dim b As String = "hello"
Dim dict As New HashMap<String, Long>
```

Converts to:
```rust
let a: i32 = 5;
let b: &str = "hello";
let mut dict: HashMap<String, i32> = HashMap::new();
```

#### Control Flow
- If/ElseIf/Else → if/else if/else
- Select Case/Case/Case Else → match
- For/To/Step → for/in with ..=
- ForEach/In → for/in with references
- While/Do/Loop → while
- Do While/Until → loop with break/while

#### Functions
```vb
Function Add(x As Integer, y As Integer) As Integer
    Function = x + y
End Function
```

Converts to:
```rust
fn add(x: i32, y: i32) -> i32 {
    x + y
}
```

#### Error Handling
- On Error Resume Next → Full explanation + error handling guidance
- As Result<T, E> → `-> Result<T, String>`
- Return Ok(value) / Return Err(msg)
- ? operator support

#### Collections
- HashMap operations (insert, get, contains_key, remove)
- Vec operations
- For Each iteration

#### String Operations
- & concatenation → format!()
- Len, Left, Right, Mid → .len(), [..3], [s.len()-3..], [1..4]
- UCase, LCase → to_uppercase(), to_lowercase()
- Trim, Replace, InStr → trim(), replace(), find()
- Val → parse::<T>()

### Not Supported (Will Generate Helpful Errors)

- Currency type (use f64 or i64 explicitly)
- Variant type (Rust requires explicit types)
- On Error GoTo (use Result<T, String> instead)
- Property Let/Get (use methods instead)
- With blocks (use explicit variable names)
- Option Base (Rust is always zero-indexed)
- Sub procedures (use Functions with no return)
- Dynamic arrays without explicit sizing
- Pointer types (use references instead)
- Custom type assignments without clone()
- Unknown-size type assignments (must use explicit clone)

## Usage

```bash
cargo build --release
./target/release/vbr_transpiler input.vb > output.rs
```

Or use it programmatically:
```rust
use vbr_transpiler::transpile;

let vb_code = "Dim x As Long = 5";
let rust_code = transpile(vb_code).unwrap();
```

## Testing

Test files are in the `tests/` directory:
- `tests/test_basic.vb` - Basic types, control flow, functions
- `tests/test_advanced.vb` - Advanced features (types, collections, error handling)
- `tests/test_errors.vb` - Features that require explicit Rust alternatives

### Running Tests

```bash
# Transpile test files
for f in tests/*.vb; do
    echo "=== $f ==="
    cargo run -- $f 2>&1 | head -20
done

# Compile transpiled output (requires Cargo.toml setup)
cargo check --example from_vbr
```

## Architecture

The transpiler consists of:

1. **lexer** (`src/lexer.rs`) - Tokenizes VB source code
2. **parser** (`src/parser.rs`) - Parses tokens into AST
3. **ast** (`src/ast.rs`) - Abstract syntax tree definitions
4. **transpiler** (`src/transpiler.rs`) - Converts AST to Rust code

## Limitations & Educational Notes

The transpiler is designed as a **teaching tool**, not a production compiler:

1. **Size Matters**: Rust knows the size of fixed types (i32, f64, bool, u8) so they copy freely. Unknown-size types (String, Vec, HashMap) require explicit cloning or borrowing.

2. **Ownership**: VB's implicit copying doesn't exist in Rust. The transpiler guides users to explicit `.clone()` or borrowing (`&`, `&mut`).

3. **Error Messages**: When Rust concepts conflict with VB patterns, the transpiler provides educational error messages explaining the Rust way.

4. **Progressive Learning**: Start with basic types and control flow, then gradually introduce collections, error handling, and advanced features.

## Future Improvements

- [ ] Verbose mode with more educational comments
- [ ] Better handling of Option Base (generate bounds checks)
- [ ] Module system support
- [ ] More sophisticated type inference
- [ ] Pattern matching conversion (Select Case → match)
- [ ] Better test infrastructure with round-trip verification