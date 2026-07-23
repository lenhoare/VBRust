// Built-in examples for the picker. They're imported straight from the repo's
// `examples/` directory as raw text (Vite `?raw`), so they never drift from the
// real, tested programs. A ⚙ marks ones that need the project runner (stdlib or
// a GUI/TUI/web target) rather than the single-file Run button.
import hello from "../../examples/hello.vbr?raw";
import strings from "../../examples/string_funcs.vbr?raw";
import match from "../../examples/match.vbr?raw";
import methods from "../../examples/methods.vbr?raw";
import sumTypes from "../../examples/sum_types.vbr?raw";
import iterators from "../../examples/iterators.vbr?raw";
import result from "../../examples/result.vbr?raw";
import vec from "../../examples/vec.vbr?raw";
import hashmap from "../../examples/hashmap.vbr?raw";
import multiDim from "../../examples/multi_dim.vbr?raw";
import inlineRust from "../../examples/inline_rust.vbr?raw";
import settings from "../../examples/settings.vbr?raw";
import tuiCounter from "../../examples/tui_counter.vbr?raw";
import webCounter from "../../examples/web_counter.vbr?raw";
import dataframe from "../../examples/dataframe_basics.vbr?raw";
import python from "../../examples/python_scalar.vbr?raw";

export interface Example {
  label: string;
  source: string;
}

export const EXAMPLES: Example[] = [
  { label: "Hello", source: hello },
  { label: "Strings", source: strings },
  { label: "Match", source: match },
  { label: "Structs & methods", source: methods },
  { label: "Enums (sum types)", source: sumTypes },
  { label: "Iterators", source: iterators },
  { label: "Result & Try", source: result },
  { label: "Vec", source: vec },
  { label: "HashMap", source: hashmap },
  { label: "Multiple Dim", source: multiDim },
  { label: "Inline Rust", source: inlineRust },
  { label: "GUI window (Iced) ⚙", source: settings },
  { label: "Terminal app (ratatui) ⚙", source: tuiCounter },
  { label: "Web page (Yew) ⚙", source: webCounter },
  { label: "DataFrames (polars) ⚙", source: dataframe },
  { label: "Python interop ⚙", source: python },
];
