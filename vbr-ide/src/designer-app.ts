import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  isDesignerDirty,
  markDesignerSaved,
  resetDesigner,
  saveDesign,
  setupDesigner,
} from "./designer";

const THEME_KEY = "vbr-ide.theme";
const ACCENT_KEY = "vbr-ide.accent";

if (localStorage.getItem(THEME_KEY) === "light") {
  document.body.classList.add("light");
}
const accent = localStorage.getItem(ACCENT_KEY);
if (accent && /^#[0-9a-fA-F]{6}$/.test(accent)) {
  document.documentElement.style.setProperty("--accent", accent);
}

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

const designerEl = document.getElementById("designer")!;
const designCodeWrap = document.getElementById("design-code-wrap")!;
const gutterDesign = document.getElementById("gutter-design")!;
let draggingDesign = false;

gutterDesign.addEventListener("mousedown", () => {
  draggingDesign = true;
  document.body.classList.add("resizing");
});
window.addEventListener("mousemove", (e) => {
  if (!draggingDesign) return;
  const rect = designerEl.getBoundingClientRect();
  const w = Math.min(rect.width * 0.7, Math.max(200, rect.right - e.clientX));
  designCodeWrap.style.flexBasis = `${w}px`;
});
window.addEventListener("mouseup", () => {
  draggingDesign = false;
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
  void win.onCloseRequested((e) => {
    if (isDesignerDirty() && !window.confirm("Discard this design? It hasn't been saved yet.")) {
      e.preventDefault();
    }
  });
} catch {
  // Browser preview has no Tauri window handle.
}
