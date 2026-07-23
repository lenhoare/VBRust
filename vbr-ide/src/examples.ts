// Built-in examples for the picker, imported straight from the repo's
// `examples/` directory as raw text (Vite `?raw`) so they never drift. A ⚙
// marks ones that need the project runner (stdlib / a GUI-TUI-web target).
import hello from "../../examples/hello.vbr?raw";
import greeting from "../../examples/greeting.vbr?raw";
import strings from "../../examples/string_funcs.vbr?raw";
import maths from "../../examples/maths.vbr?raw";
import logic from "../../examples/logic.vbr?raw";
import constants from "../../examples/constants.vbr?raw";
import conversions from "../../examples/conversions.vbr?raw";

import match from "../../examples/match.vbr?raw";
import matchGuards from "../../examples/match_guards.vbr?raw";
import doloop from "../../examples/doloop.vbr?raw";
import iterators from "../../examples/iterators.vbr?raw";

import arrays from "../../examples/arrays.vbr?raw";
import vec from "../../examples/vec.vbr?raw";
import hashmap from "../../examples/hashmap.vbr?raw";
import listLiteral from "../../examples/list_literal.vbr?raw";
import iteratorStrings from "../../examples/iterator_strings.vbr?raw";

import functions from "../../examples/functions.vbr?raw";
import methods from "../../examples/methods.vbr?raw";
import sumTypes from "../../examples/sum_types.vbr?raw";
import enums from "../../examples/enums.vbr?raw";
import enumPayloads from "../../examples/enum_payloads.vbr?raw";
import firstclass from "../../examples/firstclass_types.vbr?raw";
import multiDim from "../../examples/multi_dim.vbr?raw";

import result from "../../examples/result.vbr?raw";
import option from "../../examples/option.vbr?raw";
import resultE from "../../examples/result_e.vbr?raw";

import inlineRust from "../../examples/inline_rust.vbr?raw";
import opaqueHandle from "../../examples/opaque_handle.vbr?raw";
import rustStringMethods from "../../examples/rust_string_methods.vbr?raw";

import pythonScalar from "../../examples/python_scalar.vbr?raw";
import pythonTuple from "../../examples/python_tuple.vbr?raw";
import pythonHandle from "../../examples/python_handle.vbr?raw";

import dataframe from "../../examples/dataframe_basics.vbr?raw";
import dataframeGroupby from "../../examples/dataframe_groupby.vbr?raw";
import datetimeJson from "../../examples/datetime_json.vbr?raw";

import fetch from "../../examples/fetch.vbr?raw";
import httpPost from "../../examples/http_post.vbr?raw";
import database from "../../examples/database.vbr?raw";
import logging from "../../examples/logging.vbr?raw";
import dice from "../../examples/dice.vbr?raw";

import settings from "../../examples/settings.vbr?raw";
import guiLayout from "../../examples/gui_layout.vbr?raw";
import radioChoice from "../../examples/radio_choice.vbr?raw";
import canvas from "../../examples/canvas.vbr?raw";

import tuiCounter from "../../examples/tui_counter.vbr?raw";
import tuiInput from "../../examples/tui_input.vbr?raw";
import webCounter from "../../examples/web_counter.vbr?raw";

export interface Example {
  label: string;
  group: string;
  source: string;
}

export const EXAMPLES: Example[] = [
  { group: "Basics", label: "Hello", source: hello },
  { group: "Basics", label: "Greeting", source: greeting },
  { group: "Basics", label: "Strings", source: strings },
  { group: "Basics", label: "Maths", source: maths },
  { group: "Basics", label: "Logic & booleans", source: logic },
  { group: "Basics", label: "Constants", source: constants },
  { group: "Basics", label: "Conversions", source: conversions },

  { group: "Control flow", label: "Match", source: match },
  { group: "Control flow", label: "Match guards", source: matchGuards },
  { group: "Control flow", label: "Do / Loop", source: doloop },
  { group: "Control flow", label: "Iterators", source: iterators },

  { group: "Collections", label: "Arrays", source: arrays },
  { group: "Collections", label: "Vec", source: vec },
  { group: "Collections", label: "HashMap", source: hashmap },
  { group: "Collections", label: "List literal", source: listLiteral },
  { group: "Collections", label: "Iterating strings", source: iteratorStrings },

  { group: "Functions & types", label: "Functions", source: functions },
  { group: "Functions & types", label: "Structs & methods", source: methods },
  { group: "Functions & types", label: "Enums (sum types)", source: sumTypes },
  { group: "Functions & types", label: "Enums", source: enums },
  { group: "Functions & types", label: "Enum payloads", source: enumPayloads },
  { group: "Functions & types", label: "First-class types", source: firstclass },
  { group: "Functions & types", label: "Multiple Dim", source: multiDim },

  { group: "Result & Option", label: "Result & Try", source: result },
  { group: "Result & Option", label: "Option", source: option },
  { group: "Result & Option", label: "Result with error", source: resultE },

  { group: "Escape hatches", label: "Inline Rust", source: inlineRust },
  { group: "Escape hatches", label: "Opaque Rust handle", source: opaqueHandle },
  { group: "Escape hatches", label: "Rust string methods", source: rustStringMethods },

  { group: "Python interop", label: "Scalars ⚙", source: pythonScalar },
  { group: "Python interop", label: "Tuple return ⚙", source: pythonTuple },
  { group: "Python interop", label: "PyObject handle ⚙", source: pythonHandle },

  { group: "Data", label: "DataFrames ⚙", source: dataframe },
  { group: "Data", label: "DataFrame GroupBy ⚙", source: dataframeGroupby },
  { group: "Data", label: "DateTime & JSON ⚙", source: datetimeJson },

  { group: "Standard library", label: "HTTP fetch ⚙", source: fetch },
  { group: "Standard library", label: "HTTP POST ⚙", source: httpPost },
  { group: "Standard library", label: "SQLite ⚙", source: database },
  { group: "Standard library", label: "Logging ⚙", source: logging },
  { group: "Standard library", label: "Dice (Rnd) ⚙", source: dice },

  { group: "GUI (Window)", label: "Settings form ⚙", source: settings },
  { group: "GUI (Window)", label: "Layout ⚙", source: guiLayout },
  { group: "GUI (Window)", label: "Radio choice ⚙", source: radioChoice },
  { group: "GUI (Window)", label: "Canvas drawing ⚙", source: canvas },

  { group: "Terminal (Screen)", label: "Counter ⚙", source: tuiCounter },
  { group: "Terminal (Screen)", label: "Input list ⚙", source: tuiInput },
  { group: "Web (Page)", label: "Counter ⚙", source: webCounter },
];
