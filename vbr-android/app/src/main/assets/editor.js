(function loadTerminalFont() {
  /* System / ui-monospace at 12px for now. Unscii stays in assets/fonts/
     so we can switch back: FontFace("Unscii", "url(fonts/unscii-16.ttf)"). */
})();

const UNTITLED = `' TIDE — Turbo Pascal vibes for VBR
' File / Edit / Run / Help.  Bottom: F1 Help  F4 C  F8 Watch  F9 Run
' Ctrl+P Open project   Ctrl+U Units   Ctrl+S Save

Function Main()
    Debug.Print "Hello from TIDE"
End Function
`;

const VBR_KW = [
  "Function","End","Sub","Dim","As","If","Then","Else","ElseIf","For","To","Step",
  "Next","Each","In","Do","Loop","While","Until","Match","Return","Public","Private",
  "Type","Enum","Const","True","False","And","Or","Not","Xor","Mod","ByVal","ByRef",
  "Set","Mut","Nothing","New","Me","Exit","Continue","Use","Rust","Python","Text",
  "Test","Assert","Screen","Window","Page","State","Status","Menu","Item","Separator",
  "View","Events","Column","Row","Frame","Tabs","Tab","Space","Input","Memo","List",
  "Table","Gauge","Sparkline","BarChart","Chart","Button","Checkbox","Radio","On",
  "Every","Await","Result","Option","Ok","Err","Some","None","Integer","Long",
  "LongLong","Single","Double","Boolean","Byte","String","Vec","HashMap","Debug",
  "Print","Log","Sleep","MsgBox","InputBox","GetOpenFilename","GetSaveAsFilename",
];
const C_KW = [
  "auto","break","case","char","const","continue","default","do","double","else",
  "enum","extern","float","for","goto","if","inline","int","long","register",
  "return","short","signed","sizeof","static","struct","switch","typedef","union",
  "unsigned","void","volatile","while","bool","true","false","NULL","size_t",
];

const MENUS = {
  file: [
    ["New", "new"],
    ["Open file", "open"],
    ["Open project", "project"],
    ["Units", "units"],
    ["Examples", "examples"],
    ["Save", "save"],
    ["Save as...", "saveas"],
    ["Quit", "quit"],
  ],
  edit: [
    ["Undo", "undo"],
    ["Redo", "redo"],
    ["Cut", "cut"],
    ["Copy", "copy"],
    ["Paste", "paste"],
    ["Find", "find"],
    ["Replace", "replace"],
  ],
  run: [
    ["Compile", "compile"],
    ["Run", "run"],
    ["View C", "togglec"],
    ["View Watch", "togglewatch"],
  ],
  help: [
    ["Keys", "keys"],
    ["About", "about"],
  ],
};

const native = typeof Vbr !== "undefined" && Vbr.hasNative && Vbr.hasNative();

const S = {
  filename: "NONAME.VBR",
  uri: null,
  savedText: UNTITLED,
  project: null,
  showC: false,
  showWatch: false,
  focus: "editor",
  menuOpen: null,
  menuSel: 0,
  dialog: null,
  message: " F1 Help  F10 Menu  Ctrl+P Project  Ctrl+U Units ",
  diagnostics: [],
  watchSel: 0,
  cCode: "",
  lineMap: [],
  cStale: true,
  cCursor: 0,
  find: { query: "", replace: "", case: false, matches: [], current: -1 },
  output: "",
  blocked: null,
  hasErrors: false,
  surface: null,
  imeCss: 0,
  screen: null,
  screenMenu: null,
  screenTimers: [],
  screenTimerKey: "",
};

const editor = document.getElementById("editor");
const hl = document.getElementById("hl");
const cview = document.getElementById("cview");
const watchEl = document.getElementById("watch");
const statusEl = document.getElementById("status");
const dropdown = document.getElementById("dropdown");
const overlay = document.getElementById("overlay");
const dialogEl = document.getElementById("dialog");
const edTitle = document.getElementById("ed-title");
const cTitle = document.getElementById("c-title");
const cFrame = document.getElementById("c-frame");
const watchFrame = document.getElementById("watch-frame");
const editorFrame = document.getElementById("editor-frame");
const symbar = document.getElementById("symbar");

const SYMS = [
  ["Tab", "\t"],
  ["{", "{"],
  ["}", "}"],
  ["[", "["],
  ["]", "]"],
  ["(", "("],
  [")", ")"],
  ["<", "<"],
  [">", ">"],
  [";", ";"],
  [":", ":"],
  ["=", "="],
  ["\\", "\\"],
  ["|", "|"],
  ["&", "&"],
  ['"', '"'],
  ["'", "'"],
  ["`", "`"],
  ["_", "_"],
  ["#", "#"],
  ["$", "$"],
  ["%", "%"],
  ["^", "^"],
  ["~", "~"],
];

function insertText(s) {
  const active = document.activeElement;
  const target =
    active &&
    (active.tagName === "TEXTAREA" || active.tagName === "INPUT") &&
    typeof active.selectionStart === "number"
      ? active
      : editor;
  pushEditorUndo(target);
  target.focus();
  let ok = false;
  try {
    ok = document.execCommand("insertText", false, s);
  } catch (e) {
    ok = false;
  }
  if (!ok) {
    const start = target.selectionStart;
    const end = target.selectionEnd;
    if (typeof target.setRangeText === "function") {
      target.setRangeText(s, start, end, "end");
    } else {
      target.value = target.value.slice(0, start) + s + target.value.slice(end);
      const pos = start + s.length;
      target.setSelectionRange(pos, pos);
    }
  }
  if (target === editor) {
    markStale();
    paintEditor();
    paintC();
    paintChrome();
  }
}

const editorUndoStack = [];

function pushEditorUndo(target) {
  if (target !== editor) return;
  editorUndoStack.push({
    text: editor.value,
    start: editor.selectionStart,
    end: editor.selectionEnd,
  });
  if (editorUndoStack.length > 80) editorUndoStack.shift();
}

function editorUndo() {
  editor.focus();
  const before = editor.value;
  let undone = false;
  try {
    undone = document.execCommand("undo");
  } catch (e) {
    undone = false;
  }
  if (!undone || editor.value === before) {
    const snap = editorUndoStack.pop();
    if (snap) {
      editor.value = snap.text;
      editor.setSelectionRange(snap.start, snap.end);
    }
  }
  markStale();
  paintEditor();
  paintC();
  paintChrome();
}

function hotLabel(text) {
  const i = String(text).search(/\S/);
  if (i < 0) return esc(text);
  return (
    esc(text.slice(0, i)) +
    '<span class="hot">' +
    esc(text.charAt(i)) +
    "</span>" +
    esc(text.slice(i + 1))
  );
}

function fillSymbar() {
  if (!symbar) return;
  symbar.innerHTML = "";
  const undo = document.createElement("button");
  undo.type = "button";
  undo.className = "sym undo";
  undo.textContent = "Undo";
  undo.addEventListener("mousedown", (e) => e.preventDefault());
  undo.addEventListener("click", () => editorUndo());
  symbar.appendChild(undo);
  for (const [label, ins] of SYMS) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "sym" + (label === "Tab" ? " wide" : "");
    b.textContent = label;
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", () => insertText(ins));
    symbar.appendChild(b);
  }
}
fillSymbar();

function keyboardInset() {
  /* Prefer the Android IME overlap (already view-relative). visualViewport
     often stays full-size with adjustNothing, and taking max() with it used
     to lift the symbol bar by a nav-bar on 3-button phones. */
  if (S.imeCss > 0) return S.imeCss;
  const vv = window.visualViewport;
  return vv
    ? Math.max(0, Math.round(window.innerHeight - vv.height - vv.offsetTop))
    : 0;
}

function pinSymbar() {
  if (!symbar) return;
  if (S.screen || S.dialog) {
    symbar.classList.remove("over-kb");
    document.body.classList.remove("ime-open");
    document.documentElement.style.setProperty("--ime-inset", "0px");
    symbar.style.bottom = "";
    return;
  }
  const inset = keyboardInset();
  const over = inset > 40;
  document.body.classList.toggle("ime-open", over);
  document.documentElement.style.setProperty("--ime-inset", over ? inset + "px" : "0px");
  symbar.classList.toggle("over-kb", over);
  symbar.style.bottom = over ? inset + "px" : "";
  if (over) requestAnimationFrame(() => requestAnimationFrame(scrollCaretIntoView));
}

function scrollCaretIntoView() {
  if (!editor || S.screen || S.dialog) return;
  if (!document.body.classList.contains("ime-open")) return;
  const text = editor.value;
  const pos = editor.selectionStart || 0;
  let line = 0;
  for (let i = 0; i < pos; i++) if (text.charCodeAt(i) === 10) line += 1;
  const cs = window.getComputedStyle(editor);
  const lh = parseFloat(cs.lineHeight) || 14;
  const pad = parseFloat(cs.paddingTop) || 8;
  const y = line * lh + pad;
  const viewTop = editor.scrollTop;
  const viewBot = viewTop + editor.clientHeight;
  const margin = lh * 1.5;
  if (y < viewTop + margin) editor.scrollTop = Math.max(0, y - margin);
  else if (y + lh > viewBot - margin) {
    editor.scrollTop = Math.max(0, y + lh - editor.clientHeight + margin);
  }
  hl.scrollTop = editor.scrollTop;
  hl.scrollLeft = editor.scrollLeft;
}

window.onImeInset = function (androidPx) {
  const dpr = window.devicePixelRatio || 1;
  S.imeCss = Math.max(0, Math.round((Number(androidPx) || 0) / dpr));
  pinSymbar();
};

if (window.visualViewport) {
  window.visualViewport.addEventListener("resize", pinSymbar);
  window.visualViewport.addEventListener("scroll", pinSymbar);
}
window.addEventListener("resize", pinSymbar);
window.addEventListener("focusin", pinSymbar);
window.addEventListener("focusout", () => setTimeout(pinSymbar, 120));

function dirty() {
  return editor.value !== S.savedText;
}

function parseJson(s, fallback) {
  try { return JSON.parse(s); } catch { return fallback; }
}

function esc(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function isIdent(ch) {
  return /[A-Za-z0-9_]/.test(ch);
}

function highlightLine(line, keywords, commentPrefix, cls) {
  let out = "";
  let i = 0;
  const kw = keywords;
  while (i < line.length) {
    if (commentPrefix && line.startsWith(commentPrefix, i)) {
      out += `<span class="${cls.com}">${esc(line.slice(i))}</span>`;
      break;
    }
    const ch = line[i];
    if (ch === '"') {
      let j = i + 1;
      while (j < line.length) {
        if (line[j] === '"' && line[j + 1] === '"') { j += 2; continue; }
        if (line[j] === '"') { j += 1; break; }
        j += 1;
      }
      out += `<span class="${cls.str}">${esc(line.slice(i, j))}</span>`;
      i = j;
      continue;
    }
    if (/[A-Za-z_]/.test(ch)) {
      let j = i + 1;
      while (j < line.length && isIdent(line[j])) j += 1;
      const word = line.slice(i, j);
      const hit = kw.some((k) => k.toLowerCase() === word.toLowerCase());
      out += hit
        ? `<span class="${cls.kw}">${esc(word)}</span>`
        : esc(word);
      i = j;
      continue;
    }
    if (/[0-9]/.test(ch)) {
      let j = i + 1;
      while (j < line.length && /[0-9.]/.test(line[j])) j += 1;
      out += `<span class="${cls.num}">${esc(line.slice(i, j))}</span>`;
      i = j;
      continue;
    }
    out += esc(ch);
    i += 1;
  }
  return out;
}

function highlightVbr(src) {
  const cls = { kw: "kw", str: "str", com: "com", num: "num" };
  return src.split("\n").map((ln) => highlightLine(ln, VBR_KW, "'", cls)).join("\n");
}

function highlightCLine(ln) {
  const cls = { kw: "c-kw", str: "c-str", com: "c-com", num: "c-num" };
  return highlightLine(ln, C_KW, "//", cls);
}

function cursorPos() {
  const text = editor.value.slice(0, editor.selectionStart);
  const parts = text.split("\n");
  return { line: parts.length - 1, col: parts[parts.length - 1].length };
}

function gotoLine(line0, col0) {
  const lines = editor.value.split("\n");
  const line = Math.max(0, Math.min(line0, lines.length - 1));
  const col = Math.max(0, Math.min(col0 || 0, (lines[line] || "").length));
  let off = 0;
  for (let i = 0; i < line; i++) off += lines[i].length + 1;
  off += col;
  editor.setSelectionRange(off, off);
  setFocus("editor");
  paint();
}

function cSpanForVbr(map, vbr1, cLineCount) {
  if (!map.length || !cLineCount || !vbr1) return null;
  let start = null;
  let end = null;
  for (let i = 0; i < map.length; i++) {
    const [r1, v] = map[i];
    if (v !== vbr1) continue;
    const segEnd = map[i + 1] ? Math.max(map[i + 1][0] - 1, r1) : Math.max(cLineCount, r1);
    start = start == null ? r1 : Math.min(start, r1);
    end = end == null ? segEnd : Math.max(end, segEnd);
  }
  if (start != null) {
    const s0 = Math.max(0, start - 1);
    const e0 = Math.min(cLineCount - 1, Math.max(end - 1, s0));
    return [s0, e0];
  }
  let nearest = null;
  for (let i = map.length - 1; i >= 0; i--) {
    if (map[i][1] <= vbr1) { nearest = map[i]; break; }
  }
  if (!nearest) nearest = map[0];
  const r0 = Math.min(cLineCount - 1, Math.max(0, nearest[0] - 1));
  return [r0, r0];
}

function vbrLineForC(map, c1) {
  let last = null;
  for (const [r, v] of map) {
    if (r > c1) break;
    last = v;
  }
  return last;
}

function hideIme() {
  editor.blur();
  if (typeof Vbr !== "undefined" && Vbr.hideKeyboard) Vbr.hideKeyboard();
  setTimeout(pinSymbar, 50);
}

function showIme() {
  editor.focus();
  if (typeof Vbr !== "undefined" && Vbr.showKeyboard) Vbr.showKeyboard();
  setTimeout(pinSymbar, 50);
}

function setFocus(which) {
  S.focus = which;
  if (which === "editor") {
    /* keyboard is shown only on an actual tap in the editor */
  } else {
    hideIme();
  }
  editorFrame.classList.toggle("focused", which === "editor");
  cFrame.classList.toggle("focused", which === "c");
  watchFrame.classList.toggle("focused", which === "watch");
  paintChrome();
}

function cycleFocus(dir) {
  const order = ["editor"];
  if (S.showC) order.push("c");
  if (S.showWatch) order.push("watch");
  let i = order.indexOf(S.focus);
  if (i < 0) i = 0;
  i = (i + dir + order.length) % order.length;
  setFocus(order[i]);
}

function setMessage(t) {
  S.message = t;
}

function paintChrome() {
  const d = dirty();
  edTitle.textContent = " " + S.filename + (d ? "*" : "") + " ";
  cFrame.classList.toggle("hidden", !S.showC);
  cFrame.classList.toggle("stale", S.showC && S.cStale);
  watchFrame.classList.toggle("hidden", !S.showWatch);
  editorFrame.classList.toggle("focused", S.focus === "editor" && !S.menuOpen && !S.dialog);
  cFrame.classList.toggle("focused", S.focus === "c" && !S.menuOpen && !S.dialog);
  watchFrame.classList.toggle("focused", S.focus === "watch" && !S.menuOpen && !S.dialog);
  const cmdC = document.getElementById("cmd-c");
  if (cmdC) cmdC.classList.toggle("on", S.showC);
  const cmdW = document.getElementById("cmd-watch");
  if (cmdW) cmdW.classList.toggle("on", S.showWatch);

  const pos = cursorPos();
  const proj = S.project ? "[" + S.project.name + "] " : "";
  let unit = "";
  if (S.project && S.project.units && S.project.units.length) {
    const idx = S.project.units.findIndex((u) => u.uri === S.uri || u.name === S.filename);
    if (idx >= 0) unit = (idx + 1) + "/" + S.project.units.length + " ";
  }
  const errs = S.diagnostics.filter((x) => x.level === "error").length;
  const left =
    " " + proj + unit + S.filename + (d ? "*" : " ") +
    (errs ? "  " + errs + " error(s) " : " ");
  const focus =
    S.focus === "c" ? "C" : S.focus === "watch" ? "WATCH" : "EDIT";
  statusEl.innerHTML =
    "<span>" + esc(left) + "</span><span class='right'> " +
    focus + "  Ln " + (pos.line + 1) + ", Col " + (pos.col + 1) + " </span>";
}

function paintEditor() {
  hl.innerHTML = highlightVbr(editor.value) + "\n";
  hl.scrollTop = editor.scrollTop;
  hl.scrollLeft = editor.scrollLeft;
}

function paintC() {
  if (!S.showC) return;
  const raw = S.cCode || "/* F4 — generated C (Turbo Debugger style) */\n";
  const lines = raw.split("\n");
  const vbr1 = cursorPos().line + 1;
  let span = cSpanForVbr(S.lineMap, vbr1, lines.length);
  if (!span && lines.length) {
    const ratio = cursorPos().line / Math.max(1, editor.value.split("\n").length);
    const i = Math.min(lines.length - 1, Math.floor(ratio * lines.length));
    span = [i, i];
  }
  if (S.focus === "c") {
    span = [S.cCursor, S.cCursor];
  }
  const [a, b] = span || [-1, -1];
  cview.innerHTML = lines.map((ln, i) => {
    const cls = i >= a && i <= b ? "cline mapped" : "cline";
    return `<div class="${cls}" data-i="${i}">${highlightCLine(ln) || " "}</div>`;
  }).join("");
  const el = cview.querySelector(".mapped");
  if (el && S.focus !== "c") el.scrollIntoView({ block: "nearest" });
  if (S.focus === "c") {
    const cur = cview.querySelector(`.cline[data-i="${S.cCursor}"]`);
    if (cur) cur.scrollIntoView({ block: "nearest" });
  }
}

function paintWatch() {
  watchEl.innerHTML = "";
  if (!S.diagnostics.length) {
    const row = document.createElement("div");
    row.className = "wrow";
    row.textContent = " No diagnostics. Compile (Alt+F9) fills this list.";
    watchEl.appendChild(row);
    return;
  }
  S.diagnostics.forEach((d, i) => {
    const row = document.createElement("div");
    const sym = d.level === "error" ? "✘" : d.level === "warning" ? "⚠" : "ℹ";
    row.className = "wrow" + (i === S.watchSel ? " sel" : "") + " " + (d.level || "");
    row.textContent = d.line
      ? `${sym} [${d.line}] ${d.message}`
      : `${sym} ${d.message}`;
    row.addEventListener("click", () => {
      S.watchSel = i;
      jumpWatch();
    });
    watchEl.appendChild(row);
  });
}

function paint() {
  paintEditor();
  paintC();
  paintWatch();
  paintChrome();
}

function jumpWatch() {
  const d = S.diagnostics[S.watchSel];
  if (!d || !d.line) return;
  const col = d.range ? Math.max(0, (d.range.startColumn || 1) - 1) : 0;
  gotoLine(d.line - 1, col);
  setMessage(" Jumped to line " + d.line + ".");
}

function applyCompile(result) {
  S.cCode = result.code || "";
  S.diagnostics = result.diagnostics || [];
  S.lineMap = result.line_map || [];
  S.hasErrors = !!result.has_errors;
  S.blocked = result.blocked || null;
  S.surface = result.surface || null;
  S.cStale = false;
  if (S.watchSel >= S.diagnostics.length) S.watchSel = 0;
  if (S.hasErrors && S.diagnostics.length && S.showWatch) S.focus = "watch";
  const errs = S.diagnostics.filter((d) => d.level === "error").length;
  const mapNote = S.lineMap.length ? "" : " (no line map — proportional scroll)";
  if (result.blocked) setMessage(" " + result.blocked);
  else if (errs) {
    setMessage(
      " " + errs + " error(s)." + (S.showWatch ? " Enter in Watch jumps." : " F8 Watch.")
    );
  }
  else setMessage(" Compiled." + (S.showC ? mapNote : ""));
  paint();
}

function doCompile() {
  if (typeof Vbr === "undefined") {
    S.cCode = "/* Open this UI inside the VBR Android app to transpile. */\n";
    S.cStale = false;
    setMessage(" Editor only — native compiler not attached");
    paint();
    return;
  }
  const result = parseJson(Vbr.compile(editor.value), null);
  if (!result) {
    setMessage(" Compile returned unreadable JSON");
    return;
  }
  applyCompile(result);
}

function doRun() {
  doCompile();
  if (S.hasErrors) {
    const n = S.diagnostics.filter((d) => d.level === "error").length;
    setMessage(" Run blocked: " + n + " error(s). Fix, then F9 again.");
    if (S.showWatch) S.focus = "watch";
    paint();
    return;
  }
  if (S.surface === "screen") {
    openScreen();
    return;
  }
  if (typeof Vbr === "undefined") {
    openDialog({ kind: "output", title: " Output ", body: "Run needs the Android app." });
    return;
  }
  setMessage(" Running…");
  requestAnimationFrame(() => {
    const result = parseJson(Vbr.run(editor.value), null);
    if (!result) {
      openDialog({ kind: "output", title: " Output ", body: "Run returned unreadable JSON" });
      setMessage(" Run failed");
      return;
    }
    if (result.stage === "screen" || result.surface === "screen") {
      openScreen();
      return;
    }
    if (result.code) {
      S.cCode = result.code;
      S.cStale = false;
    }
    if (result.diagnostics) S.diagnostics = result.diagnostics;
    const body = (result.stdout || "") + (result.stderr ? (result.stdout ? "\n" : "") + result.stderr : "");
    S.output = body || (result.success ? "(no output)" : "Run failed.");
    if (result.stage === "blocked") setMessage(" Can't run this program on the phone (yet)");
    else if (result.stage === "diagnostics") setMessage(" Fix the errors, then Run");
    else if (result.success) setMessage(" Ran");
    else setMessage(" Run failed");
    openDialog({ kind: "output", title: " Output ", body: S.output });
    paint();
  });
}

function markStale() {
  if (S.showC || S.lineMap.length) S.cStale = true;
  paintChrome();
}

function toggleWatch() {
  S.showWatch = !S.showWatch;
  if (S.showWatch) {
    hideIme();
    setFocus("watch");
    setMessage(
      S.diagnostics.length
        ? " Watch on — tap a line to jump, F8 hide."
        : " Watch on — empty until Compile finds something. F8 hide."
    );
  } else {
    if (S.focus === "watch") setFocus("editor");
    hideIme();
    setMessage(" Watch off.");
  }
  paint();
}

function toggleC() {
  S.showC = !S.showC;
  if (S.showC) {
    if (S.cStale || !S.cCode) doCompile();
    hideIme();
    setFocus("c");
    const mapNote = S.lineMap.length ? "" : " (no line map — proportional scroll)";
    setMessage(" C pane on" + mapNote + " — tap editor to type, F4 hide.");
  } else {
    hideIme();
    setFocus("editor");
    setMessage(" C pane off.");
  }
  paint();
}

/* ---- menus ---- */

function closeMenu() {
  S.menuOpen = null;
  dropdown.hidden = true;
  document.querySelectorAll(".mbtn").forEach((b) => b.classList.remove("on"));
}

function openMenu(id) {
  hideIme();
  S.menuOpen = id;
  S.menuSel = 0;
  document.querySelectorAll(".mbtn").forEach((b) => {
    b.classList.toggle("on", b.dataset.menu === id);
  });
  const items = MENUS[id];
  dropdown.innerHTML = "";
  items.forEach((it, i) => {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "ditem" + (i === S.menuSel ? " sel" : "");
    b.innerHTML = hotLabel(it[0]);
    b.addEventListener("click", (e) => {
      e.stopPropagation();
      runCmd(it[1]);
    });
    dropdown.appendChild(b);
  });
  const btn = document.querySelector(`.mbtn[data-menu="${id}"]`);
  dropdown.style.left = btn ? btn.offsetLeft + "px" : "0";
  dropdown.hidden = false;
}

function runCmd(cmd) {
  closeMenu();
  switch (cmd) {
    case "new": maybeDiscard(() => doNew()); break;
    case "open": openDialog({ kind: "open" }); break;
    case "project": openDialog({ kind: "project" }); break;
    case "units": openDialog({ kind: "units" }); break;
    case "examples": openDialog({ kind: "examples" }); break;
    case "save": doSave(); break;
    case "saveas": openDialog({ kind: "saveas", name: S.filename === "NONAME.VBR" ? "" : S.filename }); break;
    case "quit": maybeQuit(); break;
    case "undo": editorUndo(); break;
    case "redo": document.execCommand("redo"); break;
    case "cut": document.execCommand("cut"); break;
    case "copy": document.execCommand("copy"); break;
    case "paste": document.execCommand("paste"); break;
    case "find": openDialog({ kind: "find" }); break;
    case "replace": openDialog({ kind: "replace" }); break;
    case "compile": doCompile(); break;
    case "run": doRun(); break;
    case "togglec": toggleC(); break;
    case "togglewatch": toggleWatch(); break;
    case "keys": openDialog({ kind: "help" }); break;
    case "about": openDialog({ kind: "about" }); break;
  }
}

document.querySelectorAll(".mbtn").forEach((b) => {
  b.innerHTML = hotLabel(b.textContent);
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    if (S.menuOpen === b.dataset.menu) closeMenu();
    else openMenu(b.dataset.menu);
  });
});

document.getElementById("cmdbar").addEventListener("click", (e) => {
  const b = e.target.closest("[data-cmd]");
  if (!b) return;
  e.preventDefault();
  runCmd(b.dataset.cmd);
});

document.addEventListener("click", () => closeMenu());

/* ---- dialogs ---- */

function closeDialog() {
  S.dialog = null;
  overlay.hidden = true;
  setFocus("editor");
}

function openDialog(d) {
  hideIme();
  closeMenu();
  S.dialog = d;
  renderDialog();
  overlay.hidden = false;
}

function renderDialog() {
  const d = S.dialog;
  if (!d) return;
  const box = dialogEl;
  const h = (title, body) => {
    box.innerHTML = `<h2>${esc(title)}</h2>${body}`;
  };

  if (d.kind === "help") {
    h(" Keys ", `<p>Bottom bar: F1 Help  F4 C  F8 Watch  F9 Run
F10  Menus          F1   This help
F9   Run            Alt+F9 Compile
     A Screen opens the TUI (tap instead of Tab/Space/Enter).
     Esc / q / Back closes it. Debug.Print runs Function Main() here
     (Android will not let TinyCC JIT into RAM). The C pane still shows
     the generated C.
F4   View C         F8   View Watch
Ctrl+P Open project Ctrl+U Units list
Ctrl+F Find         F3 / Shift+F3 next/prev
Ctrl+H Replace
Ctrl+O Open file    Ctrl+N New
Ctrl+Q Quit         Ctrl+Z/Y Undo/Redo
Tab  cycle panes    Enter jump to error
Tap the editor to type. Keyboard overlays the bottom.</p>
<div class="row"><button type="button" class="primary" data-act="close">Close</button></div>`);
  } else if (d.kind === "about") {
    h(" About ", `<p>TIDE — IDE for VBR
Turbo Pascal vibes. Android build.

F9 on a Screen runs the desktop TUI here, tap-driven.
Debug.Print runs Function Main() in that same host (TinyCC's in-process
JIT hangs on this Android). F4 still shows the generated C.
Same compiler as desktop TIDE; this phone has no rustc.</p>
<div class="row"><button type="button" class="primary" data-act="close">Close</button></div>`);
  } else if (d.kind === "output") {
    h(d.title || " Output ", `<p>${esc(d.body || "")}</p>
<div class="row"><button type="button" class="primary" data-act="close">Close</button></div>`);
  } else if (d.kind === "quit") {
    h(" Quit ", `<p>File not saved. Quit anyway?</p>
<div class="row">
  <button type="button" class="primary" data-act="quit-yes">Y = Yes</button>
  <button type="button" data-act="close">N = No</button>
</div>`);
  } else if (d.kind === "confirmnew") {
    h(" New ", `<p>File not saved. Discard and start a new file?</p>
<div class="row">
  <button type="button" class="primary" data-act="new-yes">Yes</button>
  <button type="button" data-act="close">No</button>
</div>`);
  } else if (d.kind === "find") {
    h(" Find ", `<p>Text to find</p>
<input id="dlg-find" type="text" value="${esc(S.find.query)}" />
<label class="check"><input id="dlg-cs" type="checkbox" ${S.find.case ? "checked" : ""}/> Case sensitive</label>
<p class="hint">Enter = Find   Esc = Cancel</p>
<div class="row">
  <button type="button" class="primary" data-act="find-go">Find</button>
  <button type="button" data-act="close">Cancel</button>
</div>`);
  } else if (d.kind === "replace") {
    h(" Replace ", `<p>Find</p><input id="dlg-find" type="text" value="${esc(S.find.query)}" />
<p>Replace</p><input id="dlg-repl" type="text" value="${esc(S.find.replace)}" />
<label class="check"><input id="dlg-cs" type="checkbox" ${S.find.case ? "checked" : ""}/> Case sensitive</label>
<div class="row">
  <button type="button" class="primary" data-act="repl-one">Replace</button>
  <button type="button" data-act="repl-all">Replace all</button>
  <button type="button" data-act="close">Close</button>
</div>`);
  } else if (d.kind === "saveas") {
    h(" Save as ", `<p>File name (saved in app storage), or browse the phone.</p>
<input id="dlg-name" type="text" value="${esc(d.name || "")}" placeholder="NONAME.VBR" />
<div class="row">
  <button type="button" class="primary" data-act="saveas-go">Save</button>
  <button type="button" data-act="saveas-browse">Browse…</button>
  <button type="button" data-act="close">Cancel</button>
</div>`);
  } else if (d.kind === "open") {
    const saved = typeof Vbr !== "undefined" && Vbr.listSaved
      ? parseJson(Vbr.listSaved(), []) : [];
    const list = saved.length
      ? saved.map((n) => `<button type="button" class="pick" data-open-app="${esc(n)}">${esc(n)}</button>`).join("")
      : `<p class="hint">No files in app storage yet.</p>`;
    h(" Open file ", `<p>App storage, or browse the phone.</p>
<div class="list">${list}</div>
<div class="row">
  <button type="button" class="primary" data-act="open-browse">Browse…</button>
  <button type="button" data-act="close">Cancel</button>
</div>`);
  } else if (d.kind === "project") {
    h(" Open project ", `<p>A folder with main.vbr or several .vbr files.</p>
<div class="row">
  <button type="button" class="primary" data-act="proj-app">App programs</button>
  <button type="button" data-act="proj-browse">Browse folder…</button>
  <button type="button" data-act="close">Cancel</button>
</div>`);
  } else if (d.kind === "units") {
    const units = (S.project && S.project.units) || [];
    if (!units.length) {
      h(" Units ", `<p>No .vbr units.\nOpen a project first (Ctrl+P).</p>
<div class="row"><button type="button" class="primary" data-act="close">Close</button></div>`);
    } else {
      const list = units.map((u, i) =>
        `<button type="button" class="pick${u.uri === S.uri ? " sel" : ""}" data-unit="${i}">${esc(u.name)}</button>`
      ).join("");
      h(" Units ", `<div class="list">${list}</div>
<div class="row"><button type="button" data-act="close">Cancel</button></div>`);
    }
  } else if (d.kind === "examples") {
    const names = typeof Vbr !== "undefined" && Vbr.listExamples
      ? parseJson(Vbr.listExamples(), []) : [];
    const list = names.map((n) =>
      `<button type="button" class="pick" data-ex="${esc(n)}">${esc(n.replace(/_/g, " "))}</button>`
    ).join("");
    h(" Examples ", `<div class="list">${list || "<p class='hint'>None bundled.</p>"}</div>
<div class="row"><button type="button" data-act="close">Cancel</button></div>`);
  }

  box.querySelectorAll("[data-act]").forEach((b) => {
    b.addEventListener("click", () => dialogAction(b.dataset.act));
  });
  box.querySelectorAll("[data-open-app]").forEach((b) => {
    b.addEventListener("click", () => loadAppFile(b.dataset.openApp));
  });
  box.querySelectorAll("[data-unit]").forEach((b) => {
    b.addEventListener("click", () => loadUnit(+b.dataset.unit));
  });
  box.querySelectorAll("[data-ex]").forEach((b) => {
    b.addEventListener("click", () => loadExample(b.dataset.ex));
  });
  const first = box.querySelector("input[type=text]");
  if (first) {
    first.focus();
    first.select();
    first.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        if (d.kind === "find") dialogAction("find-go");
        else if (d.kind === "replace") dialogAction("repl-one");
        else if (d.kind === "saveas") dialogAction("saveas-go");
      }
    });
  }
}

function dialogAction(act) {
  if (act === "close") { closeDialog(); return; }
  if (act === "quit-yes") {
    closeDialog();
    if (typeof Vbr !== "undefined" && Vbr.finishApp) Vbr.finishApp();
    return;
  }
  if (act === "new-yes") { closeDialog(); doNew(); return; }
  if (act === "find-go") {
    const q = document.getElementById("dlg-find").value;
    S.find.query = q;
    S.find.case = document.getElementById("dlg-cs").checked;
    closeDialog();
    findNext(1);
    return;
  }
  if (act === "repl-one" || act === "repl-all") {
    S.find.query = document.getElementById("dlg-find").value;
    S.find.replace = document.getElementById("dlg-repl").value;
    S.find.case = document.getElementById("dlg-cs").checked;
    if (act === "repl-all") replaceAll();
    else replaceOne();
    return;
  }
  if (act === "saveas-go") {
    let name = (document.getElementById("dlg-name").value || "NONAME.VBR").trim();
    if (!/\.vbr$/i.test(name)) name += ".vbr";
    closeDialog();
    saveApp(name);
    return;
  }
  if (act === "saveas-browse") {
    closeDialog();
    if (typeof Vbr !== "undefined" && Vbr.pickSaveAs) Vbr.pickSaveAs(S.filename, editor.value);
    else setMessage(" Browse needs the Android app.");
    return;
  }
  if (act === "open-browse") {
    closeDialog();
    if (typeof Vbr !== "undefined" && Vbr.pickOpenFile) Vbr.pickOpenFile();
    else setMessage(" Browse needs the Android app.");
    return;
  }
  if (act === "proj-browse") {
    closeDialog();
    if (typeof Vbr !== "undefined" && Vbr.pickProject) Vbr.pickProject();
    else setMessage(" Browse needs the Android app.");
    return;
  }
  if (act === "proj-app") {
    closeDialog();
    openAppProject();
  }
}

function maybeDiscard(fn) {
  if (!dirty()) { fn(); return; }
  S._pending = fn;
  openDialog({ kind: "confirmnew" });
}

function maybeQuit() {
  if (!dirty()) {
    if (typeof Vbr !== "undefined" && Vbr.finishApp) Vbr.finishApp();
    return;
  }
  openDialog({ kind: "quit" });
}

window.tideBack = function () {
  if (S.screen) { stopScreen(); return false; }
  if (S.dialog) { closeDialog(); return false; }
  if (S.menuOpen) { closeMenu(); return false; }
  if (dirty()) { openDialog({ kind: "quit" }); return false; }
  return true;
};

function doNew() {
  editor.value = UNTITLED;
  S.filename = "NONAME.VBR";
  S.uri = null;
  S.savedText = UNTITLED;
  S.cStale = true;
  setMessage(" New file.");
  doCompile();
}

function loadAppFile(name) {
  closeDialog();
  const text = Vbr.loadSaved(name);
  if (!text) { setMessage(" Cannot open " + name); return; }
  editor.value = text;
  S.filename = name;
  S.uri = "app:" + name;
  S.savedText = text;
  S.cStale = true;
  setMessage(" Opened " + name + ".");
  persist();
  doCompile();
}

function loadExample(name) {
  closeDialog();
  const text = Vbr.loadExample(name);
  editor.value = text;
  S.filename = name + ".vbr";
  S.uri = "asset:" + name;
  S.savedText = text;
  S.cStale = true;
  setMessage(" Example " + name + " (read-only — Save as to keep).");
  persist();
  doCompile();
}

function loadUnit(i) {
  const u = S.project.units[i];
  closeDialog();
  if (!u) return;
  let text = "";
  if (u.uri.startsWith("app:")) text = Vbr.loadSaved(u.name);
  else text = Vbr.readUri(u.uri);
  if (text == null) { setMessage(" Cannot open " + u.name); return; }
  editor.value = text;
  S.filename = u.name;
  S.uri = u.uri;
  S.savedText = text;
  S.cStale = true;
  setMessage(" Unit " + u.name + ".");
  persist();
  doCompile();
}

function saveApp(name) {
  if (typeof Vbr === "undefined" || !Vbr.saveProgram) {
    setMessage(" Save needs the Android app.");
    return;
  }
  Vbr.saveProgram(name, editor.value);
  S.filename = name;
  S.uri = "app:" + name;
  S.savedText = editor.value;
  setMessage(" Saved " + name + ".");
  persist();
  paintChrome();
}

function doSave() {
  if (S.uri && S.uri.startsWith("app:")) {
    saveApp(S.filename);
    return;
  }
  if (S.uri && S.uri.startsWith("content:") && typeof Vbr !== "undefined" && Vbr.writeUri) {
    const err = Vbr.writeUri(S.uri, editor.value);
    if (err && err !== "ok") { setMessage(" " + err); return; }
    S.savedText = editor.value;
    setMessage(" Saved " + S.filename + ".");
    persist();
    paintChrome();
    return;
  }
  if (S.uri && S.uri.startsWith("asset:")) {
    openDialog({ kind: "saveas", name: S.filename });
    return;
  }
  openDialog({ kind: "saveas", name: S.filename === "NONAME.VBR" ? "" : S.filename });
}

function openAppProject() {
  const names = typeof Vbr !== "undefined" ? parseJson(Vbr.listSaved(), []) : [];
  const units = names.map((n) => ({ name: n, uri: "app:" + n }));
  units.sort((a, b) => {
    const am = a.name.toLowerCase() === "main.vbr";
    const bm = b.name.toLowerCase() === "main.vbr";
    if (am !== bm) return am ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
  S.project = { kind: "app", uri: "app:", name: "programs", units };
  const entry = units.find((u) => u.name.toLowerCase() === "main.vbr") || units[0];
  setMessage(" Project [programs] — " + units.length + " unit(s).");
  persist();
  if (entry) loadUnit(units.indexOf(entry));
  else paintChrome();
}

function persist() {
  if (typeof Vbr === "undefined" || !Vbr.persistSession) return;
  Vbr.persistSession(JSON.stringify({
    filename: S.filename,
    uri: S.uri,
    project: S.project,
  }));
}

window.onPickedFile = function (json) {
  const info = typeof json === "string" ? parseJson(json, null) : json;
  if (!info) return;
  const text = Vbr.readUri(info.uri);
  editor.value = text;
  S.filename = info.name || "file.vbr";
  S.uri = info.uri;
  S.savedText = text;
  S.cStale = true;
  setMessage(" Opened " + S.filename + ".");
  persist();
  doCompile();
};

window.onPickedProject = function (json) {
  const info = typeof json === "string" ? parseJson(json, null) : json;
  if (!info) return;
  S.project = {
    kind: "tree",
    uri: info.uri,
    name: info.name || "project",
    units: info.units || [],
  };
  setMessage(" Project [" + S.project.name + "] — " + S.project.units.length + " unit(s).");
  persist();
  const units = S.project.units;
  const entry = units.find((u) => u.name.toLowerCase() === "main.vbr") || units[0];
  if (entry) {
    const text = Vbr.readUri(entry.uri);
    editor.value = text;
    S.filename = entry.name;
    S.uri = entry.uri;
    S.savedText = text;
    S.cStale = true;
    doCompile();
  } else {
    setMessage(" No .vbr units in that folder.");
    paintChrome();
  }
};

window.onSavedAs = function (json) {
  const info = typeof json === "string" ? parseJson(json, null) : json;
  if (!info) return;
  S.filename = info.name || S.filename;
  S.uri = info.uri;
  S.savedText = editor.value;
  setMessage(" Saved " + S.filename + ".");
  persist();
  paintChrome();
};

window.onNativeError = function (msg) {
  setMessage(" " + msg);
};

/* ---- find / replace ---- */

function findAll() {
  const q = S.find.query;
  S.find.matches = [];
  if (!q) return;
  const src = editor.value;
  const hay = S.find.case ? src : src.toLowerCase();
  const needle = S.find.case ? q : q.toLowerCase();
  let i = 0;
  while (i <= hay.length - needle.length) {
    const at = hay.indexOf(needle, i);
    if (at < 0) break;
    S.find.matches.push([at, at + q.length]);
    i = at + Math.max(1, q.length);
  }
}

function findNext(dir) {
  findAll();
  if (!S.find.matches.length) {
    setMessage(" Search string not found.");
    return;
  }
  const pos = editor.selectionStart;
  let idx = S.find.current;
  if (dir > 0) {
    idx = S.find.matches.findIndex((m) => m[0] >= pos && (S.find.current < 0 || m[0] > pos));
    if (idx < 0) idx = 0;
  } else {
    idx = -1;
    for (let i = S.find.matches.length - 1; i >= 0; i--) {
      if (S.find.matches[i][0] < pos) { idx = i; break; }
    }
    if (idx < 0) idx = S.find.matches.length - 1;
  }
  S.find.current = idx;
  const [a, b] = S.find.matches[idx];
  editor.setSelectionRange(a, b);
  setFocus("editor");
  setMessage(" " + (idx + 1) + " of " + S.find.matches.length);
  paint();
}

function replaceOne() {
  findAll();
  const sel = editor.value.slice(editor.selectionStart, editor.selectionEnd);
  const ok = S.find.case
    ? sel === S.find.query
    : sel.toLowerCase() === S.find.query.toLowerCase();
  if (ok && S.find.query) {
    const start = editor.selectionStart;
    editor.setRangeText(S.find.replace, start, editor.selectionEnd, "end");
    markStale();
  }
  findNext(1);
}

function replaceAll() {
  if (!S.find.query) return;
  findAll();
  const n = S.find.matches.length;
  if (!n) { setMessage(" Search string not found."); return; }
  const q = S.find.query;
  const flags = S.find.case ? "g" : "gi";
  const re = new RegExp(q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), flags);
  editor.value = editor.value.replace(re, S.find.replace);
  closeDialog();
  markStale();
  setMessage(" Replaced " + n + " occurrence(s).");
  paintEditor();
  paintChrome();
}

/* ---- events ---- */

editor.addEventListener("input", () => {
  markStale();
  paintEditor();
  paintChrome();
  scrollCaretIntoView();
});
editor.addEventListener("scroll", () => {
  hl.scrollTop = editor.scrollTop;
  hl.scrollLeft = editor.scrollLeft;
});
editor.addEventListener("click", () => {
  setFocus("editor");
  showIme();
  paintC();
  paintChrome();
  pinSymbar();
  scrollCaretIntoView();
});
editor.addEventListener("keyup", () => { paintC(); paintChrome(); scrollCaretIntoView(); });
editor.addEventListener("mouseup", () => { paintC(); paintChrome(); scrollCaretIntoView(); });

cview.addEventListener("click", (e) => {
  const row = e.target.closest(".cline");
  if (!row) return;
  S.cCursor = +row.dataset.i;
  setFocus("c");
  if (S.lineMap.length) {
    const v = vbrLineForC(S.lineMap, S.cCursor + 1);
    if (v) {
      const pos = cursorPos();
      if (pos.line !== v - 1) gotoLine(v - 1, pos.col);
    }
  } else {
    const ratio = S.cCursor / Math.max(1, cview.querySelectorAll(".cline").length);
    const v0 = Math.min(
      editor.value.split("\n").length - 1,
      Math.floor(ratio * editor.value.split("\n").length)
    );
    gotoLine(v0, cursorPos().col);
    setFocus("c");
  }
  paint();
});

document.addEventListener("keydown", (e) => {
  if (S.screen) {
    handleScreenKey(e);
    return;
  }
  if (S.dialog) {
    if (e.key === "Escape") { e.preventDefault(); closeDialog(); }
    return;
  }
  if (S.menuOpen) {
    const items = MENUS[S.menuOpen];
    if (e.key === "Escape") { e.preventDefault(); closeMenu(); return; }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      S.menuSel = (S.menuSel + 1) % items.length;
      openMenu(S.menuOpen);
      dropdown.querySelectorAll(".ditem").forEach((el, i) => el.classList.toggle("sel", i === S.menuSel));
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      S.menuSel = (S.menuSel - 1 + items.length) % items.length;
      dropdown.querySelectorAll(".ditem").forEach((el, i) => el.classList.toggle("sel", i === S.menuSel));
      return;
    }
    if (e.key === "Enter") { e.preventDefault(); runCmd(items[S.menuSel][1]); return; }
    if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
      e.preventDefault();
      const order = ["file", "edit", "run", "help"];
      let i = order.indexOf(S.menuOpen);
      i = (i + (e.key === "ArrowRight" ? 1 : -1) + 4) % 4;
      openMenu(order[i]);
      return;
    }
  }

  const ctrl = e.ctrlKey || e.metaKey;
  if (e.key === "F10") { e.preventDefault(); S.menuOpen ? closeMenu() : openMenu("file"); return; }
  if (e.key === "F1") { e.preventDefault(); runCmd("keys"); return; }
  if (e.key === "F9") { e.preventDefault(); runCmd(e.altKey ? "compile" : "run"); return; }
  if (e.key === "F4") { e.preventDefault(); runCmd("togglec"); return; }
  if (e.key === "F8") { e.preventDefault(); runCmd("togglewatch"); return; }
  if (e.key === "F3") { e.preventDefault(); findNext(e.shiftKey ? -1 : 1); return; }
  if (e.key === "Tab") {
    e.preventDefault();
    cycleFocus(e.shiftKey ? -1 : 1);
    paint();
    return;
  }
  if (ctrl && e.key.toLowerCase() === "s") { e.preventDefault(); runCmd("save"); return; }
  if (ctrl && e.key.toLowerCase() === "o") { e.preventDefault(); runCmd("open"); return; }
  if (ctrl && e.key.toLowerCase() === "n") { e.preventDefault(); runCmd("new"); return; }
  if (ctrl && e.key.toLowerCase() === "p") { e.preventDefault(); runCmd("project"); return; }
  if (ctrl && e.key.toLowerCase() === "u") { e.preventDefault(); runCmd("units"); return; }
  if (ctrl && e.key.toLowerCase() === "f") { e.preventDefault(); runCmd("find"); return; }
  if (ctrl && e.key.toLowerCase() === "h") { e.preventDefault(); runCmd("replace"); return; }
  if (ctrl && e.key.toLowerCase() === "q") { e.preventDefault(); runCmd("quit"); return; }

  if (S.focus === "watch") {
    if (e.key === "ArrowDown") { e.preventDefault(); S.watchSel = Math.min(S.diagnostics.length - 1, S.watchSel + 1); paintWatch(); return; }
    if (e.key === "ArrowUp") { e.preventDefault(); S.watchSel = Math.max(0, S.watchSel - 1); paintWatch(); return; }
    if (e.key === "Enter") { e.preventDefault(); jumpWatch(); return; }
  }
  if (S.focus === "c") {
    const n = cview.querySelectorAll(".cline").length;
    if (e.key === "ArrowDown") { e.preventDefault(); S.cCursor = Math.min(n - 1, S.cCursor + 1); paintC(); return; }
    if (e.key === "ArrowUp") { e.preventDefault(); S.cCursor = Math.max(0, S.cCursor - 1); paintC(); return; }
  }
});

overlay.addEventListener("click", (e) => {
  if (e.target === overlay) closeDialog();
});

/* ---- start ---- */

function restore() {
  if (typeof Vbr === "undefined" || !Vbr.restoreSession) return false;
  const ses = parseJson(Vbr.restoreSession(), null);
  if (!ses || !ses.uri) return false;
  S.filename = ses.filename || S.filename;
  S.uri = ses.uri;
  S.project = ses.project || null;
  let text = "";
  try {
    if (ses.uri.startsWith("app:")) text = Vbr.loadSaved(ses.filename);
    else if (ses.uri.startsWith("asset:")) text = Vbr.loadExample(ses.filename.replace(/\.vbr$/i, ""));
    else text = Vbr.readUri(ses.uri);
  } catch {
    return false;
  }
  if (!text) return false;
  editor.value = text;
  S.savedText = text;
  setMessage(" Restored " + S.filename + ".");
  return true;
}

editor.value = UNTITLED;
S.savedText = UNTITLED;
if (!restore()) {
  editor.value = UNTITLED;
  S.savedText = UNTITLED;
}
editor.setSelectionRange(0, 0);
setMessage("");
doCompile();
setFocus("editor");
hideIme();
pinSymbar();

/* ---- Screen host: desktop TUI chrome, tap instead of Tab/Space/Enter ---- */

const scHost = document.getElementById("screen-host");
const scMenubar = document.getElementById("sc-menubar");
const scTitle = document.getElementById("sc-title");
const scView = document.getElementById("sc-view");
const scStatus = document.getElementById("sc-status");

function openScreen() {
  if (typeof Vbr === "undefined" || !Vbr.screenStart) {
    openDialog({
      kind: "output",
      title: " Screen ",
      body: "Screen host needs the native library (rebuild the APK).",
    });
    return;
  }
  hideIme();
  closeMenu();
  closeDialog();
  const frame = parseJson(Vbr.screenStart(editor.value), null);
  if (!frame || !frame.ok) {
    openDialog({
      kind: "output",
      title: " Screen ",
      body: (frame && frame.error) || "Couldn't start this Screen.",
    });
    setMessage(" Screen failed to start");
    return;
  }
  S.screen = frame;
  S.screenMenu = null;
  scHost.hidden = false;
  paintScreen();
  startScreenTimers(frame);
  setMessage(" Screen running — tap a control. Esc / Back closes.");
}

function stopScreen() {
  clearScreenTimers();
  if (typeof Vbr !== "undefined" && Vbr.screenStop) Vbr.screenStop();
  S.screen = null;
  S.screenMenu = null;
  scHost.hidden = true;
  setMessage(" Screen closed.");
}

function clearScreenTimers() {
  S.screenTimers.forEach((id) => clearInterval(id));
  S.screenTimers = [];
  S.screenTimerKey = "";
}

function startScreenTimers(frame) {
  const next = (frame.timers || []).map((t) => t.ms + ":" + t.handler).join("|");
  if (S.screenTimerKey === next) return;
  clearScreenTimers();
  S.screenTimerKey = next;
  (frame.timers || []).forEach((t) => {
    const id = setInterval(() => {
      if (!S.screen) return;
      dispatchScreen({ op: "event", name: t.handler });
    }, Math.max(50, t.ms || 100));
    S.screenTimers.push(id);
  });
}

function dispatchScreen(op, opts) {
  if (typeof Vbr === "undefined" || !Vbr.screenDispatch) return;
  const frame = parseJson(Vbr.screenDispatch(JSON.stringify(op)), null);
  if (opts && opts.silent) return;
  applyScreenFrame(frame);
}

function applyScreenFrame(frame) {
  if (!frame) {
    setMessage(" Screen event returned junk");
    return;
  }
  if (frame.quit) {
    stopScreen();
    setMessage(" Screen quit.");
    return;
  }
  S.screen = frame;
  if (frame.error) setMessage(" " + frame.error);
  paintScreen();
  startScreenTimers(frame);
}

function paintScreen() {
  const f = S.screen;
  if (!f) return;
  scTitle.textContent = " " + (f.title || "Screen") + " ";
  paintScreenMenu(f);
  scView.innerHTML = "";
  scView.appendChild(renderSc(f.view));
  paintScreenStatus(f);
}

function paintScreenMenu(f) {
  scMenubar.innerHTML = "";
  const menus = f.menu || [];
  if (!menus.length) return;
  menus.forEach((m, i) => {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "sc-mbtn" + (S.screenMenu === i ? " on" : "");
    b.textContent = " " + m.title + " ";
    b.addEventListener("click", (e) => {
      e.stopPropagation();
      S.screenMenu = S.screenMenu === i ? null : i;
      paintScreen();
    });
    scMenubar.appendChild(b);
    if (S.screenMenu === i) {
      const drop = document.createElement("div");
      drop.id = "sc-dropdown";
      drop.style.left = b.offsetLeft + "px";
      (m.items || []).forEach((it) => {
        if (it.sep) {
          const sep = document.createElement("div");
          sep.className = "sc-dsep";
          drop.appendChild(sep);
          return;
        }
        const row = document.createElement("button");
        row.type = "button";
        row.className = "sc-ditem";
        row.textContent = " " + it.label;
        row.addEventListener("click", (e) => {
          e.stopPropagation();
          S.screenMenu = null;
          dispatchScreen({ op: "menu", handler: it.handler });
        });
        drop.appendChild(row);
      });
      scMenubar.appendChild(drop);
    }
  });
}

function paintScreenStatus(f) {
  scStatus.innerHTML = "";
  const stat = document.createElement("span");
  stat.className = "sc-stat";
  stat.textContent = f.status ? " " + f.status : "";
  scStatus.appendChild(stat);
  const keys = f.keys || [];
  keys.forEach((k) => {
    scStatus.appendChild(scKeyChip(k.key, k.label, () => {
      dispatchScreen({ op: "key", handler: k.handler });
    }));
  });
  const hasEsc = keys.some((k) => /^esc$/i.test(k.key) || /^escape$/i.test(k.key));
  const hasQuit = keys.some((k) => /^quit$/i.test(k.handler));
  if (!hasEsc) {
    scStatus.appendChild(scKeyChip("Esc", hasQuit ? "quit" : "close", () => {
      dispatchScreen({ op: "quit" });
    }));
  }
}

function scKeyChip(key, label, fn) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "sc-key";
  const kbd = document.createElement("kbd");
  kbd.textContent = key;
  b.appendChild(kbd);
  b.appendChild(document.createTextNode(label || ""));
  b.addEventListener("click", fn);
  return b;
}

function applyScSize(el, size, axis) {
  if (!size) {
    el.classList.add("sc-len");
    return;
  }
  const n = size.n || 1;
  if (size.kind === "fill") {
    el.classList.add("sc-fill");
    if (n > 1) el.style.flexGrow = String(n);
  } else if (size.kind === "length") {
    el.classList.add("sc-len");
    const dim = n * 1.35 + "em";
    if (axis === "row") el.style.width = dim;
    else el.style.height = dim;
  } else if (size.kind === "percent") {
    el.classList.add("sc-pct");
    if (axis === "row") el.style.flex = "0 0 " + n + "%";
    else el.style.flex = "0 0 " + n + "%";
  } else if (size.kind === "min") {
    el.classList.add("sc-min");
    if (axis === "row") el.style.minWidth = n * 1.35 + "em";
    else el.style.minHeight = n * 1.35 + "em";
  }
}

function renderSc(node, axis) {
  if (!node || typeof node !== "object") return document.createTextNode("");
  const wrap = (el) => {
    applyScSize(el, node.size, axis);
    return el;
  };
  switch (node.kind) {
    case "column":
    case "row": {
      const el = document.createElement("div");
      el.className = node.kind === "row" ? "sc-row" : "sc-col";
      if (node.spacing) el.style.gap = node.spacing * 0.6 + "em";
      if (node.padding) el.style.padding = node.padding * 0.6 + "em";
      const childAxis = node.kind === "row" ? "row" : "col";
      (node.children || []).forEach((c) => el.appendChild(renderSc(c, childAxis)));
      return wrap(el);
    }
    case "frame": {
      const el = document.createElement("div");
      el.className = "sc-framebox";
      if (node.title) {
        const t = document.createElement("div");
        t.className = "sc-frame-title";
        t.textContent = " " + node.title + " ";
        el.appendChild(t);
      }
      const body = document.createElement("div");
      body.className = "sc-col";
      if (node.spacing) body.style.gap = node.spacing * 0.6 + "em";
      (node.children || []).forEach((c) => body.appendChild(renderSc(c, "col")));
      el.appendChild(body);
      return wrap(el);
    }
    case "space": {
      const el = document.createElement("div");
      if (node.horizontal) el.style.width = (node.amount || 1) + "ch";
      else el.style.height = (node.amount || 1) * 1.2 + "em";
      return wrap(el);
    }
    case "text": {
      const el = document.createElement("div");
      el.className = "sc-text";
      el.textContent = node.text || "";
      return wrap(el);
    }
    case "button": {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "sc-btn";
      b.textContent = "[ " + (node.label || "Button") + " ]";
      b.addEventListener("click", () => {
        if (node.handler) dispatchScreen({ op: "click", handler: node.handler });
      });
      return wrap(b);
    }
    case "checkbox": {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "sc-check";
      b.textContent = "[" + (node.value ? "x" : " ") + "] " + (node.label || "");
      b.addEventListener("click", () => {
        dispatchScreen({ op: "toggle", field: node.field, handler: node.handler || "" });
      });
      return wrap(b);
    }
    case "radio": {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "sc-radio";
      b.textContent = "(" + (node.selected ? "*" : " ") + ") " + (node.label || "");
      b.addEventListener("click", () => {
        dispatchScreen({
          op: "radio",
          field: node.field,
          option: node.option,
          handler: node.handler || "",
        });
      });
      return wrap(b);
    }
    case "input": {
      const inp = document.createElement("input");
      inp.type = "text";
      inp.className = "sc-input";
      inp.value = node.value || "";
      inp.placeholder = node.placeholder || "";
      inp.dataset.scField = node.field || "";
      inp.addEventListener("input", () => {
        dispatchScreen({ op: "input", field: node.field, value: inp.value }, { silent: true });
      });
      inp.addEventListener("keydown", (e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          dispatchScreen({ op: "input", field: node.field, value: inp.value }, { silent: true });
          if (node.handler) dispatchScreen({ op: "submit", field: node.field, handler: node.handler });
        }
      });
      return wrap(inp);
    }
    case "memo": {
      const ta = document.createElement("textarea");
      ta.className = "sc-memo";
      ta.value = node.value || "";
      ta.dataset.scField = node.field || "";
      ta.addEventListener("input", () => {
        dispatchScreen({ op: "input", field: node.field, value: ta.value }, { silent: true });
      });
      return wrap(ta);
    }
    case "list": {
      const el = document.createElement("div");
      el.className = "sc-list";
      (node.items || []).forEach((item, i) => {
        const b = document.createElement("button");
        b.type = "button";
        b.className = "sc-li" + (i === node.selected ? " sel" : "");
        b.textContent = " " + item;
        b.addEventListener("click", () => {
          dispatchScreen({
            op: "list",
            field: node.field,
            index: i,
            handler: node.handler || "",
          });
        });
        el.appendChild(b);
      });
      return wrap(el);
    }
    case "tabs": {
      const el = document.createElement("div");
      el.className = "sc-tabs";
      const bar = document.createElement("div");
      bar.className = "sc-tabbar";
      const body = document.createElement("div");
      body.className = "sc-tabbody";
      (node.tabs || []).forEach((t, i) => {
        const b = document.createElement("button");
        b.type = "button";
        b.className = "sc-tab" + (i === node.index ? " on" : "");
        b.textContent = " " + t.title + " ";
        b.addEventListener("click", () => {
          dispatchScreen({
            op: "tab",
            field: node.field,
            index: i,
            handler: node.handler || "",
          });
        });
        bar.appendChild(b);
        if (i === node.index) {
          (t.body || []).forEach((c) => body.appendChild(renderSc(c, "col")));
        }
      });
      el.appendChild(bar);
      el.appendChild(body);
      return wrap(el);
    }
    case "gauge": {
      const el = document.createElement("div");
      el.className = "sc-gauge";
      const i = document.createElement("i");
      i.style.width = Math.round((node.pct || 0) * 100) + "%";
      const s = document.createElement("span");
      s.textContent = String(Math.round(node.value != null ? node.value : 0));
      el.appendChild(i);
      el.appendChild(s);
      return wrap(el);
    }
    case "sparkline": {
      const el = document.createElement("div");
      el.className = "sc-spark";
      const vals = node.values || [];
      const max = Math.max(1, ...vals.map((n) => Math.abs(n)));
      vals.forEach((n) => {
        const b = document.createElement("b");
        b.style.height = Math.max(2, Math.round((Math.abs(n) / max) * 100)) + "%";
        el.appendChild(b);
      });
      return wrap(el);
    }
    case "slider": {
      const inp = document.createElement("input");
      inp.type = "range";
      inp.className = "sc-slider";
      inp.min = node.min;
      inp.max = node.max;
      inp.value = node.value;
      inp.addEventListener("change", () => {
        dispatchScreen({
          op: "slider",
          field: node.field,
          value: Number(inp.value),
          handler: node.handler || "",
        });
      });
      return wrap(inp);
    }
    case "empty":
      return wrap(document.createElement("div"));
    case "unsupported": {
      const el = document.createElement("div");
      el.className = "sc-unsup";
      el.textContent = (node.hint || node.widget || "widget") + "";
      return wrap(el);
    }
    default: {
      const el = document.createElement("div");
      el.className = "sc-unsup";
      el.textContent = node.kind || "";
      return wrap(el);
    }
  }
}

function screenKeyName(e) {
  const map = {
    Escape: "Esc",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Enter: "Enter",
    " ": "Space",
    Tab: "Tab",
    Backspace: "Backspace",
    Delete: "Delete",
  };
  if (map[e.key]) return map[e.key];
  if (e.key.length === 1) return e.key;
  return e.key;
}

function handleScreenKey(e) {
  const typing =
    e.target &&
    (e.target.tagName === "INPUT" || e.target.tagName === "TEXTAREA");
  if (e.key === "Escape") {
    e.preventDefault();
    if (S.screenMenu != null) {
      S.screenMenu = null;
      paintScreen();
      return;
    }
    const keys = (S.screen && S.screen.keys) || [];
    const esc = keys.find((k) => /^esc$/i.test(k.key) || /^escape$/i.test(k.key));
    if (esc) dispatchScreen({ op: "key", handler: esc.handler });
    else dispatchScreen({ op: "quit" });
    return;
  }
  if (e.key === "F10") {
    e.preventDefault();
    const menus = (S.screen && S.screen.menu) || [];
    if (!menus.length) return;
    S.screenMenu = S.screenMenu == null ? 0 : null;
    paintScreen();
    return;
  }
  if (typing) return;
  const name = screenKeyName(e);
  const keys = (S.screen && S.screen.keys) || [];
  const hit = keys.find((k) => k.key === name || k.key.toLowerCase() === name.toLowerCase());
  if (hit) {
    e.preventDefault();
    dispatchScreen({ op: "key", handler: hit.handler });
  }
}

scHost.addEventListener("click", () => {
  if (S.screenMenu != null) {
    S.screenMenu = null;
    paintScreen();
  }
});

