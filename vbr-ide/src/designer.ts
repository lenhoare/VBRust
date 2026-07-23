import { invoke } from "@tauri-apps/api/core";
import type * as monaco from "monaco-editor";

// A widget tree, built visually and sent to the compiler core to become VBR
// `View` code. `id` is frontend-only bookkeeping — the Rust side ignores it.
interface DProps {
  text?: string;
  field?: string;
  event?: string;
  width?: string;
  spacing?: number;
  padding?: number;
  name?: string;
  w?: number;
  h?: number;
  min?: number;
  max?: number;
}
interface DNode {
  id: number;
  kind: string;
  props: DProps;
  children: DNode[];
}

const CONTAINERS = new Set(["Column", "Row"]);
const PALETTE = [
  "Column", "Row",
  "Text", "Button", "TextInput", "TextArea", "Checkbox", "Toggler", "Slider",
  "ProgressBar", "Image", "Space", "Canvas", "Chart",
];

let uid = 1;
const nextId = () => uid++;

function defaults(kind: string): DProps {
  switch (kind) {
    case "Column":
    case "Row":
      return { spacing: 8, padding: 8 };
    case "Text":
      return { text: "Label" };
    case "Button":
      return { text: "Button", event: "Clicked" };
    case "TextInput":
      return { text: "Type here…", field: "value", event: "Typed" };
    case "TextArea":
      return { field: "notes" };
    case "Checkbox":
      return { text: "Check me", field: "checked", event: "Toggled" };
    case "Toggler":
      return { text: "Toggle me", field: "on", event: "Toggled" };
    case "Slider":
      return { field: "amount", event: "Changed", min: 0, max: 100 };
    case "ProgressBar":
      return { field: "level", min: 0, max: 100 };
    case "Image":
      return { text: "assets/logo.png" };
    case "Space":
      return { h: 20 };
    case "Canvas":
    case "Chart":
      return { name: kind === "Chart" ? "myChart" : "myCanvas", w: 300, h: 200 };
    default:
      return {};
  }
}

let root: DNode = { id: nextId(), kind: "Column", props: { spacing: 8, padding: 16 }, children: [] };
let selectedId = root.id;

let paletteItemsEl: HTMLElement;
let surfaceEl: HTMLElement;
let propsEl: HTMLElement;
let codeEl: HTMLElement;
let editorRef: monaco.editor.IStandaloneCodeEditor;

function findNode(
  id: number,
  node: DNode = root,
  parent: DNode | null = null,
): { node: DNode; parent: DNode | null } | null {
  if (node.id === id) return { node, parent };
  for (const c of node.children) {
    const found = findNode(id, c, node);
    if (found) return found;
  }
  return null;
}

// Where a new control lands: inside the selected container, else beside the
// selection, else the root.
function targetContainer(): DNode {
  const sel = findNode(selectedId);
  if (!sel) return root;
  if (CONTAINERS.has(sel.node.kind)) return sel.node;
  return sel.parent ?? root;
}

function addControl(kind: string): void {
  const node: DNode = { id: nextId(), kind, props: defaults(kind), children: [] };
  targetContainer().children.push(node);
  selectedId = node.id;
  render();
}

function deleteSelected(): void {
  if (selectedId === root.id) return;
  const sel = findNode(selectedId);
  if (!sel || !sel.parent) return;
  sel.parent.children = sel.parent.children.filter((c) => c.id !== selectedId);
  selectedId = sel.parent.id;
  render();
}

// ---- surface rendering ----------------------------------------------------

function widgetEl(node: DNode): HTMLElement {
  const p = node.props;
  const el = document.createElement("div");
  el.className = "dnode";
  el.dataset.id = String(node.id);

  if (CONTAINERS.has(node.kind)) {
    el.classList.add("container", node.kind === "Row" ? "row" : "col");
    const tag = document.createElement("div");
    tag.className = "dnode-tag";
    tag.textContent = node.kind;
    tag.style.flexBasis = "100%";
    el.appendChild(tag);
    for (const c of node.children) el.appendChild(widgetEl(c));
  } else {
    const w = document.createElement("div");
    w.className = "dwidget";
    switch (node.kind) {
      case "Text":
        w.textContent = p.field ? `{${p.field}}` : p.text ?? "";
        break;
      case "Button":
        w.classList.add("button");
        w.textContent = p.text ?? "Button";
        break;
      case "TextInput":
        w.textContent = `▭ ${p.text ?? ""}`;
        break;
      case "TextArea":
        w.textContent = "▭ text area";
        w.style.minHeight = "40px";
        break;
      case "Checkbox":
        w.textContent = `☐ ${p.text ?? ""}`;
        break;
      case "Toggler":
        w.textContent = `⬤ ${p.text ?? ""}`;
        break;
      case "Slider":
        w.textContent = `━●━ ${p.field ?? ""} (${p.min ?? 0}–${p.max ?? 100})`;
        break;
      case "ProgressBar":
        w.textContent = `▰▰▱ ${p.field ?? ""}`;
        break;
      case "Image":
        w.textContent = `🖼 ${p.text ?? ""}`;
        break;
      case "Space":
        w.textContent = `↕ Space ${p.h ?? 20}`;
        w.style.opacity = "0.5";
        break;
      case "Canvas":
      case "Chart":
        w.classList.add("placeholder");
        w.textContent = `${node.kind}: ${p.name ?? ""} (${p.w ?? 0}×${p.h ?? 0})`;
        break;
      default:
        w.textContent = node.kind;
    }
    el.appendChild(w);
  }

  if (node.id === selectedId) el.classList.add("selected");
  el.addEventListener("click", (ev) => {
    ev.stopPropagation();
    selectedId = node.id;
    render();
  });
  return el;
}

// Rebuild the surface + regenerate code, but leave the properties panel alone
// (so an input keeps focus while you type into it).
function refreshLive(): void {
  surfaceEl.innerHTML = "";
  surfaceEl.appendChild(widgetEl(root));
  void regenerate();
}

function render(): void {
  refreshLive();
  renderProps();
}

// ---- properties panel -----------------------------------------------------

function renderProps(): void {
  propsEl.innerHTML = "";
  const sel = findNode(selectedId);
  if (!sel) {
    propsEl.textContent = "Select a control.";
    return;
  }
  const node = sel.node;
  const p = node.props;

  const header = document.createElement("div");
  header.className = "dnode-tag";
  header.style.marginBottom = "8px";
  header.textContent = node.kind;
  propsEl.appendChild(header);

  const field = (
    label: string,
    key: keyof DProps,
    type: "text" | "number" = "text",
    placeholder = "",
  ) => {
    const row = document.createElement("div");
    row.className = "prop-row";
    const lab = document.createElement("label");
    lab.textContent = label;
    const inp = document.createElement("input");
    inp.type = type;
    inp.placeholder = placeholder;
    const cur = p[key];
    inp.value = cur === undefined ? "" : String(cur);
    inp.addEventListener("input", () => {
      if (type === "number") {
        const n = Number(inp.value);
        (p as Record<string, unknown>)[key] = inp.value === "" || Number.isNaN(n) ? undefined : n;
      } else {
        (p as Record<string, unknown>)[key] = inp.value === "" ? undefined : inp.value;
      }
      refreshLive();
    });
    row.append(lab, inp);
    propsEl.appendChild(row);
  };

  const RANGE = ["Slider", "ProgressBar"];
  const k = node.kind;
  if (["Text", "Button", "TextInput", "Checkbox", "Toggler", "Image"].includes(k)) field("Text", "text");
  if (["Text", "TextInput", "TextArea", "Checkbox", "Toggler", ...RANGE].includes(k))
    field("Field", "field");
  if (["Button", "TextInput", "Checkbox", "Toggler", "Slider"].includes(k)) field("Event", "event");
  if (RANGE.includes(k)) {
    field("Min", "min", "number");
    field("Max", "max", "number");
  }
  if (k === "Space") field("Height", "h", "number");
  if (k === "Canvas" || k === "Chart") {
    field("Name", "name");
    field("Width", "w", "number");
    field("Height", "h", "number");
  }
  if (CONTAINERS.has(k)) {
    field("Spacing", "spacing", "number");
    field("Padding", "padding", "number");
  }
  if (node.id !== root.id) field("Size", "width", "text", "Fill / Fill 2 / Length 40");
}

// ---- codegen + insert -----------------------------------------------------

async function regenerate(): Promise<void> {
  try {
    codeEl.textContent = await invoke<string>("generate_design", { tree: root });
  } catch (e) {
    codeEl.textContent = String(e);
  }
}

function insertIntoEditor(): void {
  const vbr = codeEl.textContent ?? "";
  const model = editorRef.getModel();
  if (!model) return;
  const end = model.getFullModelRange().getEndPosition();
  editorRef.executeEdits("designer", [
    {
      range: {
        startLineNumber: end.lineNumber,
        startColumn: end.column,
        endLineNumber: end.lineNumber,
        endColumn: end.column,
      },
      text: "\n" + vbr + "\n",
    },
  ]);
  document.body.classList.remove("designer-mode");
  editorRef.focus();
}

/** Wire up the designer UI. Idempotent enough for a single call at startup. */
export function setupDesigner(editor: monaco.editor.IStandaloneCodeEditor): void {
  editorRef = editor;
  paletteItemsEl = document.getElementById("palette-items")!;
  surfaceEl = document.getElementById("surface")!;
  propsEl = document.getElementById("props")!;
  codeEl = document.getElementById("design-code")!;

  for (const kind of PALETTE) {
    const b = document.createElement("button");
    b.className = "palette-item";
    b.textContent = kind;
    b.addEventListener("click", () => addControl(kind));
    paletteItemsEl.appendChild(b);
  }
  document.getElementById("del-node")!.addEventListener("click", deleteSelected);
  document.getElementById("insert-design")!.addEventListener("click", insertIntoEditor);

  render();
}
