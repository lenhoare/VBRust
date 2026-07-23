import type * as Monaco from "monaco-editor";

export const VBR_LANGUAGE_ID = "vbr";

// These lists mirror the real keyword table in `src/lexer.rs`. When the language
// grows a keyword, add it here too — this is the one deliberate "shadow" of the
// lexer, and it only affects colour, never meaning. (Later, LSP semantic tokens
// can drive colour from the compiler itself and retire the drift entirely.)
const KEYWORDS = [
  // Declarations & flow — the hard keywords the lexer recognises.
  "Function", "Sub", "Return", "ByVal", "ByRef", "On", "Type", "Enum",
  "Public", "Private", "Const", "ReDim", "End", "Dim", "Set", "Mut", "As",
  "If", "Then", "ElseIf", "Else", "Select", "Case", "Match", "Await",
  "For", "Each", "In", "To", "Step", "Next", "New", "Do", "Loop",
  "While", "Until", "Exit", "Continue", "With",
  "And", "Or", "Not", "Xor", "Mod",
  "True", "False",
  // Soft structural words — idents to the lexer, but they read as keywords and
  // head the GUI/TUI/web/module constructs.
  "Screen", "Window", "Page", "State", "View", "Events", "Use", "Me", "Nothing",
];

const TYPE_KEYWORDS = [
  // The built-in VB scalar types.
  "Integer", "Long", "LongLong", "Single", "Double", "Boolean", "Byte",
  "String", "Currency", "Variant",
  // Soft type names that appear in declarations.
  "Vec", "HashMap", "Map", "Result", "Option", "DateTime", "DataFrame", "PyObject",
];

/**
 * Register the `vbr` language with Monaco: its comment/bracket configuration and
 * a Monarch tokeniser for colour. Idempotent, so hot-reload doesn't double-register.
 */
export function registerVbrLanguage(monaco: typeof Monaco): void {
  if (monaco.languages.getLanguages().some((l) => l.id === VBR_LANGUAGE_ID)) {
    return;
  }
  monaco.languages.register({ id: VBR_LANGUAGE_ID });

  monaco.languages.setLanguageConfiguration(VBR_LANGUAGE_ID, {
    comments: { lineComment: "'" },
    brackets: [
      ["(", ")"],
      ["[", "]"],
      ["{", "}"],
    ],
    autoClosingPairs: [
      { open: "(", close: ")" },
      { open: "[", close: "]" },
      { open: "{", close: "}" },
      { open: '"', close: '"' },
    ],
    surroundingPairs: [
      { open: "(", close: ")" },
      { open: '"', close: '"' },
    ],
  });

  const language: Monaco.languages.IMonarchLanguage = {
    ignoreCase: true, // VB is case-insensitive: dim = Dim = DIM.
    keywords: KEYWORDS,
    typeKeywords: TYPE_KEYWORDS,
    operators: ["=", "<>", "<", ">", "<=", ">=", "+", "-", "*", "/", "\\", "^", "&", "=>", ":"],

    tokenizer: {
      root: [
        // Verbatim blocks open with the bare block word and run until `End X`;
        // their interior is another language (Rust/CSS/Python) or free text, so
        // don't tokenise it as VBR. Guards keep member access (`.Rust`) and the
        // `Text "widget"` form from opening a block by mistake.
        [/\b(?:Rust|Css)\b(?=\s*$)/, { token: "keyword", next: "@verbatimRustCss" }],
        [/\bPython\b(?=\s*(\(|$))/, { token: "keyword", next: "@verbatimPython" }],
        [/\bText\b(?=\s*$)/, { token: "keyword", next: "@verbatimText" }],

        { include: "@whitespace" },

        // Identifiers, keywords, types.
        [
          /[a-zA-Z_]\w*/,
          {
            cases: {
              "@keywords": "keyword",
              "@typeKeywords": "type",
              "@default": "identifier",
            },
          },
        ],

        // Numbers.
        [/\d+\.\d+([eE][-+]?\d+)?/, "number.float"],
        [/\d+/, "number"],

        // Strings.
        [/"/, { token: "string.quote", next: "@string" }],

        // Brackets, operators, delimiters.
        [/[()\[\]{}]/, "@brackets"],
        [
          /<>|<=|>=|=>|[=<>+\-*/\\^&:]/,
          { cases: { "@operators": "operator", "@default": "" } },
        ],
        [/[,.]/, "delimiter"],
      ],

      whitespace: [
        [/[ \t\r\n]+/, ""],
        [/'.*$/, "comment"],
      ],

      string: [
        [/""/, "string.escape"], // VB uses a doubled quote for a literal quote.
        [/[^"]+/, "string"],
        [/"/, { token: "string.quote", next: "@pop" }],
      ],

      verbatimRustCss: [
        [/^\s*End\s+(?:Rust|Css)\b.*$/, { token: "keyword", next: "@pop" }],
        [/.*$/, "string"],
      ],
      verbatimPython: [
        [/^\s*End\s+Python\b.*$/, { token: "keyword", next: "@pop" }],
        [/.*$/, "string"],
      ],
      verbatimText: [
        [/^\s*End\s+Text\b.*$/, { token: "keyword", next: "@pop" }],
        [/.*$/, "string"],
      ],
    },
  };

  monaco.languages.setMonarchTokensProvider(VBR_LANGUAGE_ID, language);
}
