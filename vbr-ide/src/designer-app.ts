import * as monaco from "monaco-editor";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  bindGeneratedCode,
  isDesignerDirty,
  markDesignerSaved,
  resetDesigner,
  saveDesign,
  setupDesigner,
} from "./designer";
import { registerVbrLanguage, VBR_LANGUAGE_ID } from "./vbrLanguage";

self.MonacoEnvironment = {
  getWorker: () => new editorWorker(),
};

const THEME_KEY = "vbr-ide.theme";
const ACCENT_KEY = "vbr-ide.accent";
const ACCENT_DEFAULT = "#5ec8c5";

if (localStorage.getItem(THEME_KEY) === "light") {
  document.body.classList.add("light");
}
const accent = localStorage.getItem(ACCENT_KEY);
if (accent && /^#[0-9a-fA-F]{6}$/.test(accent)) {
  document.documentElement.style.setProperty("--accent", accent);
}

registerVbrLanguage(monaco);
const accentColor =
  getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() ||
  ACCENT_DEFAULT;
monaco.editor.defineTheme("bust-dark", {
  base: "vs-dark",
  inherit: true,
  rules: [],
  colors: {
    "editor.background": "#1e1f22",
    "editor.lineHighlightBackground": `${accentColor}22`,
    "editor.selectionBackground": `${accentColor}44`,
    "editorCursor.foreground": accentColor,
    "editorLineNumber.activeForeground": accentColor,
  },
});
monaco.editor.defineTheme("bust-light", {
  base: "vs",
  inherit: true,
  rules: [],
  colors: {
    "editor.background": "#ffffff",
    "editor.lineHighlightBackground": `${accentColor}18`,
    "editor.selectionBackground": `${accentColor}33`,
    "editorCursor.foreground": accentColor,
    "editorLineNumber.activeForeground": accentColor,
  },
});
const codeView = monaco.editor.create(document.getElementById("design-code")!, {
  value: "",
  language: VBR_LANGUAGE_ID,
  theme: document.body.classList.contains("light") ? "bust-light" : "bust-dark",
  readOnly: true,
  minimap: { enabled: false },
  fontSize: 13,
  automaticLayout: true,
  scrollBeyondLastLine: false,
  mouseWheelZoom: true,
  wordWrap: "on",
  renderLineHighlight: "none",
});
bindGeneratedCode((text) => codeView.setValue(text));

const params = new URLSearchParams(location.search);
let projectRoot = params.get("root") ?? "";
const initialTarget = params.get("target") === "tui" ? "tui" : "gui";

async function writeForm(tree: unknown, target: string): Promise<void> {
  if (!projectRoot) {
    window.alert("Open a project folder first — that's where the file is saved.");
    return;
  }
  try {
    const created = await invoke<{ path: string; name: string }>("create_form", {
      dir: projectRoot,
      tree,
      target,
    });
    markDesignerSaved();
    await emit("designer-form-created", created);
  } catch (e) {
    window.alert(String(e));
  }
}

setupDesigner(writeForm);
resetDesigner(initialTarget);

document.getElementById("save-design")!.addEventListener("click", () => {
  void saveDesign();
});

window.addEventListener("keydown", (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
    e.preventDefault();
    void saveDesign();
  }
});

const designerBody = document.getElementById("designer-body")!;
const designerEl = document.getElementById("designer")!;
const designCodeWrap = document.getElementById("design-code-wrap")!;
const gutterDesign = document.getElementById("gutter-design")!;
const gutterPalette = document.getElementById("gutter-palette")!;
let draggingDesign = false;
let draggingPalette = false;

gutterDesign.addEventListener("mousedown", () => {
  draggingDesign = true;
  document.body.classList.add("resizing");
});
gutterPalette.addEventListener("mousedown", () => {
  draggingPalette = true;
  document.body.classList.add("resizing");
});
window.addEventListener("mousemove", (e) => {
  if (draggingDesign) {
    const rect = designerEl.getBoundingClientRect();
    const w = Math.min(rect.width * 0.7, Math.max(200, rect.right - e.clientX));
    designCodeWrap.style.flexBasis = `${w}px`;
  }
  if (draggingPalette) {
    const rect = designerBody.getBoundingClientRect();
    const w = Math.min(rect.width * 0.5, Math.max(140, e.clientX - rect.left));
    document.body.style.setProperty("--sidebar-width", `${w}px`);
  }
});
window.addEventListener("mouseup", () => {
  if (draggingDesign || draggingPalette) codeView.layout();
  draggingDesign = false;
  draggingPalette = false;
  document.body.classList.remove("resizing");
});

void listen<{ target: string; root: string }>("designer-open", (e) => {
  if (isDesignerDirty() && !window.confirm("Discard this design? It hasn't been saved yet.")) {
    return;
  }
  projectRoot = e.payload.root;
  resetDesigner(e.payload.target === "tui" ? "tui" : "gui");
});

try {
  const win = getCurrentWebviewWindow();
  // Showing a modal during `closeRequested` cancels Tauri's own close. Always
  // take over the event, then `destroy()` after the user confirms (or if the
  // design is already saved) and hand focus back to the editor.
  let closing = false;
  void win.onCloseRequested(async (event) => {
    if (closing) return;
    event.preventDefault();
    if (isDesignerDirty() && !window.confirm("Discard this design? It hasn't been saved yet.")) {
      return;
    }
    closing = true;
    await emit("designer-closing");
    await win.destroy();
  });
} catch {
  // Browser preview has no Tauri window handle.
}
