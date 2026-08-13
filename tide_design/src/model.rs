//! Structural screen tree — the designer’s source of truth.

use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

fn fresh_id() -> usize {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Column,
    Row,
    Frame,
    Text,
    Space,
    Button,
    Checkbox,
    Radio,
    Input,
    List,
    Table,
    Gauge,
    Sparkline,
    BarChart,
    Chart,
    Tabs,
    Tab,
    Memo,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Column => "Column",
            Kind::Row => "Row",
            Kind::Frame => "Frame",
            Kind::Text => "Text",
            Kind::Space => "Space",
            Kind::Button => "Button",
            Kind::Checkbox => "Checkbox",
            Kind::Radio => "Radio",
            Kind::Input => "Input",
            Kind::List => "List",
            Kind::Table => "Table",
            Kind::Gauge => "Gauge",
            Kind::Sparkline => "Sparkline",
            Kind::BarChart => "BarChart",
            Kind::Chart => "Chart",
            Kind::Tabs => "Tabs",
            Kind::Tab => "Tab",
            Kind::Memo => "Memo",
        }
    }

    pub fn is_container(self) -> bool {
        matches!(self, Kind::Column | Kind::Row | Kind::Frame | Kind::Tabs | Kind::Tab)
    }

    /// Palette order for Screen widgets.
    pub fn palette() -> &'static [Kind] {
        &[
            Kind::Column,
            Kind::Row,
            Kind::Frame,
            Kind::Tabs,
            Kind::Tab,
            Kind::Text,
            Kind::Space,
            Kind::Button,
            Kind::Checkbox,
            Kind::Radio,
            Kind::Input,
            Kind::Memo,
            Kind::List,
            Kind::Table,
            Kind::Gauge,
            Kind::Sparkline,
            Kind::BarChart,
            Kind::Chart,
        ]
    }

    /// Sensible main-axis size when the user leaves size on Auto.
    pub fn auto_size(self) -> SizeHint {
        match self {
            Kind::Column | Kind::Row | Kind::Frame | Kind::Tabs | Kind::Tab => SizeHint::Fill,
            Kind::Text | Kind::Space | Kind::Button | Kind::Checkbox | Kind::Radio => {
                SizeHint::Length(1)
            }
            Kind::Input => SizeHint::Length(3),
            Kind::Gauge | Kind::Sparkline | Kind::BarChart => SizeHint::Length(3),
            Kind::Memo | Kind::List | Kind::Table | Kind::Chart => SizeHint::Fill,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SizeHint {
    Default,
    Fill,
    #[allow(dead_code)]
    FillN(u32),
    Length(u32),
    Percent(u32),
    Min(u32),
}

impl SizeHint {
    const CYCLE: &'static [SizeHint] = &[
        SizeHint::Default,
        SizeHint::Fill,
        SizeHint::Length(1),
        SizeHint::Length(3),
        SizeHint::Length(5),
        SizeHint::Length(8),
        SizeHint::Length(12),
        SizeHint::Percent(30),
        SizeHint::Percent(50),
        SizeHint::Min(5),
    ];

    pub fn as_vbr_line(&self) -> Option<String> {
        match self {
            SizeHint::Default => None,
            SizeHint::Fill => Some("Fill".into()),
            SizeHint::FillN(n) => Some(format!("Fill {n}")),
            SizeHint::Length(n) => Some(format!("Length {n}")),
            SizeHint::Percent(n) => Some(format!("Percent {n}")),
            SizeHint::Min(n) => Some(format!("Min {n}")),
        }
    }

    pub fn label(&self) -> String {
        match self {
            SizeHint::Default => "Auto".into(),
            other => other.as_vbr_line().unwrap_or_else(|| "Auto".into()),
        }
    }

    fn cycle_index(&self) -> usize {
        Self::CYCLE
            .iter()
            .position(|s| s == self)
            .unwrap_or(0)
    }

    pub fn cycle(&self) -> SizeHint {
        let i = self.cycle_index();
        Self::CYCLE[(i + 1) % Self::CYCLE.len()].clone()
    }

    pub fn cycle_back(&self) -> SizeHint {
        let i = self.cycle_index();
        let n = Self::CYCLE.len();
        Self::CYCLE[(i + n - 1) % n].clone()
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: usize,
    pub kind: Kind,
    pub text: String,
    pub field: String,
    pub event: String,
    /// Radio option expression (`0`, `Size.Small`, …). Unused on other kinds.
    pub option: String,
    pub size: SizeHint,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(kind: Kind) -> Self {
        let (text, field, event) = defaults_for(kind);
        Self {
            id: fresh_id(),
            kind,
            text,
            field,
            event,
            option: if kind == Kind::Radio {
                "0".into()
            } else {
                String::new()
            },
            size: SizeHint::Default,
            children: Vec::new(),
        }
    }

    /// Size used for layout / emit (Auto → kind-specific default).
    pub fn effective_size(&self) -> SizeHint {
        match &self.size {
            SizeHint::Default => self.kind.auto_size(),
            other => other.clone(),
        }
    }

    pub fn tree_label(&self) -> String {
        match self.kind {
            Kind::Text if !self.text.is_empty() => format!("Text \"{}\"", self.text),
            Kind::Text if !self.field.is_empty() => format!("Text {}", self.field),
            Kind::Frame if !self.text.is_empty() => format!("Frame \"{}\"", self.text),
            Kind::Tab if !self.text.is_empty() => format!("Tab \"{}\"", self.text),
            Kind::Tabs if !self.field.is_empty() => format!("Tabs {}", self.field),
            Kind::Button if !self.text.is_empty() => format!("Button \"{}\"", self.text),
            Kind::Checkbox if !self.text.is_empty() => format!("Checkbox \"{}\"", self.text),
            Kind::Radio if !self.text.is_empty() => format!("Radio \"{}\"", self.text),
            Kind::Space => match self.effective_size() {
                SizeHint::Length(n) => format!("Space Height {n}"),
                _ => "Space".into(),
            },
            Kind::Input
            | Kind::Memo
            | Kind::List
            | Kind::Table
            | Kind::Gauge
            | Kind::Sparkline
            | Kind::BarChart
            | Kind::Chart
            | Kind::Checkbox
            | Kind::Radio
                if !self.field.is_empty() =>
            {
                format!("{} {}", self.kind.label(), self.field)
            }
            _ => self.kind.label().to_string(),
        }
    }
}

fn defaults_for(kind: Kind) -> (String, String, String) {
    match kind {
        Kind::Column | Kind::Row | Kind::Space => (String::new(), String::new(), String::new()),
        Kind::Frame => ("Panel".into(), String::new(), String::new()),
        Kind::Tabs => (String::new(), "tab".into(), String::new()),
        Kind::Tab => ("Page".into(), String::new(), String::new()),
        Kind::Text => ("Label".into(), String::new(), String::new()),
        Kind::Button => ("OK".into(), String::new(), "Clicked".into()),
        Kind::Checkbox => ("Remember me".into(), "checked".into(), "Toggled".into()),
        Kind::Radio => ("Option".into(), "choice".into(), "Picked".into()),
        Kind::Input => (String::new(), "input".into(), "Submitted".into()),
        Kind::Memo => (String::new(), "notes".into(), String::new()),
        Kind::List => (String::new(), "items".into(), "Selected".into()),
        Kind::Table => (String::new(), "rows".into(), "Selected".into()),
        Kind::Gauge => (String::new(), "level".into(), String::new()),
        Kind::Sparkline => (String::new(), "series".into(), String::new()),
        Kind::BarChart => (String::new(), "bars".into(), String::new()),
        Kind::Chart => (String::new(), "curve".into(), String::new()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Bar,
    Menu,
    Item,
    Separator,
}

impl MenuKind {
    pub fn label(self) -> &'static str {
        match self {
            MenuKind::Bar => "Menu",
            MenuKind::Menu => "Menu",
            MenuKind::Item => "Item",
            MenuKind::Separator => "Separator",
        }
    }

    pub fn palette() -> &'static [MenuKind] {
        &[MenuKind::Menu, MenuKind::Item, MenuKind::Separator]
    }

    pub fn is_container(self) -> bool {
        matches!(self, MenuKind::Bar | MenuKind::Menu)
    }
}

#[derive(Debug, Clone)]
pub struct MenuNode {
    pub id: usize,
    pub kind: MenuKind,
    pub text: String,
    pub event: String,
    pub children: Vec<MenuNode>,
}

impl MenuNode {
    pub fn bar() -> Self {
        Self {
            id: fresh_id(),
            kind: MenuKind::Bar,
            text: String::new(),
            event: String::new(),
            children: Vec::new(),
        }
    }

    pub fn new(kind: MenuKind) -> Self {
        let (text, event) = match kind {
            MenuKind::Bar => (String::new(), String::new()),
            MenuKind::Menu => ("File".into(), String::new()),
            MenuKind::Item => ("Item".into(), "DoItem".into()),
            MenuKind::Separator => (String::new(), String::new()),
        };
        Self {
            id: fresh_id(),
            kind,
            text,
            event,
            children: Vec::new(),
        }
    }

    pub fn tree_label(&self) -> String {
        match self.kind {
            MenuKind::Bar => "Menu".into(),
            MenuKind::Menu if !self.text.is_empty() => format!("Menu \"{}\"", self.text),
            MenuKind::Item if !self.text.is_empty() => {
                if self.event.is_empty() {
                    format!("Item \"{}\"", self.text)
                } else {
                    format!("Item \"{}\" {}", self.text, self.event)
                }
            }
            MenuKind::Separator => "────────".into(),
            _ => self.kind.label().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Design {
    pub screen_name: String,
    pub title: String,
    pub root: Node,
    pub selected: usize,
    pub menu_root: MenuNode,
    pub menu_selected: usize,
    pub dirty: bool,
    pub path: Option<std::path::PathBuf>,
}

impl Default for Design {
    fn default() -> Self {
        let mut root = Node::new(Kind::Column);
        root.children.push(Node::new(Kind::Text));
        let menu_root = MenuNode::bar();
        let menu_selected = menu_root.id;
        let selected = root.id;
        Self {
            screen_name: "Screen1".into(),
            title: "Screen1".into(),
            root,
            selected,
            menu_root,
            menu_selected,
            dirty: false,
            path: None,
        }
    }
}

impl Design {
    pub fn selected_node(&self) -> Option<&Node> {
        find(&self.root, self.selected)
    }

    pub fn selected_node_mut(&mut self) -> Option<&mut Node> {
        let id = self.selected;
        find_mut(&mut self.root, id)
    }

    /// Flat depth-first list for the tree pane: (id, depth, label).
    pub fn flat_tree(&self) -> Vec<(usize, usize, String)> {
        let mut out = Vec::new();
        walk_flat(&self.root, 0, &mut out);
        out
    }

    pub fn select_next(&mut self, delta: isize) {
        let flat = self.flat_tree();
        if flat.is_empty() {
            return;
        }
        let i = flat
            .iter()
            .position(|(id, _, _)| *id == self.selected)
            .unwrap_or(0) as isize;
        let n = flat.len() as isize;
        let mut j = i + delta;
        while j < 0 {
            j += n;
        }
        self.selected = flat[(j as usize) % flat.len()].0;
    }

    pub fn add_child(&mut self, kind: Kind) -> bool {
        match kind {
            Kind::Tab => self.add_tab_pane(),
            Kind::Tabs => {
                if !self.insert_node(Node::new(Kind::Tabs)) {
                    return false;
                }
                let tabs_id = self.selected;
                self.push_under(tabs_id, Node::new(Kind::Tab))
            }
            other => {
                let parent_id = self.insert_parent_id();
                if find(&self.root, parent_id).is_some_and(|p| p.kind == Kind::Tabs) {
                    self.add_into_tabs(parent_id, other)
                } else {
                    self.insert_node(Node::new(other))
                }
            }
        }
    }

    fn add_tab_pane(&mut self) -> bool {
        let tab = Node::new(Kind::Tab);
        if let Some(n) = self.selected_node() {
            if n.kind == Kind::Tabs {
                return self.push_under(n.id, tab);
            }
            if n.kind == Kind::Tab {
                return self.insert_after_id(n.id, tab);
            }
            if let Some(tid) = ancestor_kind(&self.root, self.selected, Kind::Tab) {
                return self.insert_after_id(tid, tab);
            }
            if let Some(tid) = ancestor_kind(&self.root, self.selected, Kind::Tabs) {
                return self.push_under(tid, tab);
            }
        }
        self.add_child(Kind::Tabs)
    }

    fn add_into_tabs(&mut self, tabs_id: usize, kind: Kind) -> bool {
        let last_tab = find(&self.root, tabs_id).and_then(|t| {
            t.children
                .iter()
                .rev()
                .find(|c| c.kind == Kind::Tab)
                .map(|c| c.id)
        });
        let tab_id = if let Some(id) = last_tab {
            id
        } else {
            let tab = Node::new(Kind::Tab);
            let id = tab.id;
            if !self.push_under(tabs_id, tab) {
                return false;
            }
            id
        };
        self.push_under(tab_id, Node::new(kind))
    }

    fn insert_node(&mut self, child: Node) -> bool {
        let child_id = child.id;
        let parent_id = self.insert_parent_id();
        if let Some(parent) = find_mut(&mut self.root, parent_id) {
            if parent.kind.is_container() {
                parent.children.push(child);
                self.selected = child_id;
                self.dirty = true;
                return true;
            }
        }
        // Selection is a leaf — insert as sibling after it under its parent.
        if let Some((pid, idx)) = parent_of(&self.root, self.selected) {
            if let Some(parent) = find_mut(&mut self.root, pid) {
                parent.children.insert(idx + 1, child);
                self.selected = child_id;
                self.dirty = true;
                return true;
            }
        }
        false
    }

    fn push_under(&mut self, parent_id: usize, node: Node) -> bool {
        let node_id = node.id;
        if let Some(parent) = find_mut(&mut self.root, parent_id) {
            if parent.kind.is_container() {
                parent.children.push(node);
                self.selected = node_id;
                self.dirty = true;
                return true;
            }
        }
        false
    }

    fn insert_after_id(&mut self, sibling_id: usize, node: Node) -> bool {
        let node_id = node.id;
        let Some((pid, idx)) = parent_of(&self.root, sibling_id) else {
            return false;
        };
        if let Some(parent) = find_mut(&mut self.root, pid) {
            parent.children.insert(idx + 1, node);
            self.selected = node_id;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    fn insert_parent_id(&self) -> usize {
        if let Some(n) = self.selected_node() {
            if n.kind.is_container() {
                return n.id;
            }
        }
        self.selected
    }

    pub fn remove_selected(&mut self) -> bool {
        if self.selected == self.root.id {
            return false;
        }
        let Some((pid, idx)) = parent_of(&self.root, self.selected) else {
            return false;
        };
        let parent = find_mut(&mut self.root, pid).unwrap();
        parent.children.remove(idx);
        self.selected = pid;
        self.dirty = true;
        true
    }

    pub fn move_sibling(&mut self, delta: isize) -> bool {
        let Some((pid, idx)) = parent_of(&self.root, self.selected) else {
            return false;
        };
        let parent = find_mut(&mut self.root, pid).unwrap();
        let n = parent.children.len();
        if n < 2 {
            return false;
        }
        let j = idx as isize + delta;
        if j < 0 || j >= n as isize {
            return false;
        }
        parent.children.swap(idx, j as usize);
        self.dirty = true;
        true
    }

    /// Alt+Left — move node out to grandparent, after its parent.
    pub fn move_out(&mut self) -> bool {
        if self.selected == self.root.id {
            return false;
        }
        let Some((pid, idx)) = parent_of(&self.root, self.selected) else {
            return false;
        };
        if pid == self.root.id {
            return false;
        }
        let Some((gpid, pidx)) = parent_of(&self.root, pid) else {
            return false;
        };
        let parent = find_mut(&mut self.root, pid).unwrap();
        let node = parent.children.remove(idx);
        let node_id = node.id;
        let gp = find_mut(&mut self.root, gpid).unwrap();
        gp.children.insert(pidx + 1, node);
        self.selected = node_id;
        self.dirty = true;
        true
    }

    /// Alt+Right — nest inside the preceding sibling if that sibling is a container.
    pub fn move_in(&mut self) -> bool {
        let Some((pid, idx)) = parent_of(&self.root, self.selected) else {
            return false;
        };
        if idx == 0 {
            return false;
        }
        let parent = find_mut(&mut self.root, pid).unwrap();
        if !parent.children[idx - 1].kind.is_container() {
            return false;
        }
        let node = parent.children.remove(idx);
        let node_id = node.id;
        parent.children[idx - 1].children.push(node);
        self.selected = node_id;
        self.dirty = true;
        true
    }

    pub fn has_menus(&self) -> bool {
        !self.menu_root.children.is_empty()
    }

    pub fn menu_selected_node(&self) -> Option<&MenuNode> {
        find_menu(&self.menu_root, self.menu_selected)
    }

    pub fn menu_selected_node_mut(&mut self) -> Option<&mut MenuNode> {
        let id = self.menu_selected;
        find_menu_mut(&mut self.menu_root, id)
    }

    pub fn menu_flat_tree(&self) -> Vec<(usize, usize, String)> {
        let mut out = Vec::new();
        walk_menu_flat(&self.menu_root, 0, &mut out);
        out
    }

    pub fn menu_select_next(&mut self, delta: isize) {
        let flat = self.menu_flat_tree();
        if flat.is_empty() {
            return;
        }
        let i = flat
            .iter()
            .position(|(id, _, _)| *id == self.menu_selected)
            .unwrap_or(0) as isize;
        let n = flat.len() as isize;
        let mut j = i + delta;
        while j < 0 {
            j += n;
        }
        self.menu_selected = flat[(j as usize) % flat.len()].0;
    }

    pub fn menu_add(&mut self, kind: MenuKind) -> bool {
        match kind {
            MenuKind::Bar => false,
            MenuKind::Menu => self.menu_add_menu(),
            MenuKind::Item | MenuKind::Separator => self.menu_add_entry(kind),
        }
    }

    fn menu_add_menu(&mut self) -> bool {
        let mut menu = MenuNode::new(MenuKind::Menu);
        if self.menu_root.children.iter().any(|c| c.text == "File") {
            menu.text = "Edit".into();
        }
        let id = menu.id;
        if let Some(n) = self.menu_selected_node() {
            if n.kind == MenuKind::Menu {
                return self.menu_insert_after(n.id, menu);
            }
            if n.kind != MenuKind::Bar {
                if let Some((pid, _)) = parent_of_menu(&self.menu_root, n.id) {
                    if pid == self.menu_root.id {
                        return self.menu_insert_after(n.id, menu);
                    }
                    return self.menu_insert_after(pid, menu);
                }
            }
        }
        self.menu_root.children.push(menu);
        self.menu_selected = id;
        self.dirty = true;
        true
    }

    fn menu_add_entry(&mut self, kind: MenuKind) -> bool {
        let entry = MenuNode::new(kind);
        if let Some(n) = self.menu_selected_node() {
            if n.kind == MenuKind::Menu {
                return self.menu_push_under(n.id, entry);
            }
            if n.kind == MenuKind::Item || n.kind == MenuKind::Separator {
                return self.menu_insert_after(n.id, entry);
            }
        }
        if self.menu_root.children.is_empty() {
            let mut file = MenuNode::new(MenuKind::Menu);
            file.children.push(entry);
            let eid = file.children[0].id;
            self.menu_root.children.push(file);
            self.menu_selected = eid;
            self.dirty = true;
            return true;
        }
        let last = self.menu_root.children.last().unwrap().id;
        self.menu_push_under(last, entry)
    }

    fn menu_push_under(&mut self, parent_id: usize, node: MenuNode) -> bool {
        let node_id = node.id;
        if let Some(parent) = find_menu_mut(&mut self.menu_root, parent_id) {
            if parent.kind.is_container() {
                parent.children.push(node);
                self.menu_selected = node_id;
                self.dirty = true;
                return true;
            }
        }
        false
    }

    fn menu_insert_after(&mut self, sibling_id: usize, node: MenuNode) -> bool {
        let node_id = node.id;
        let Some((pid, idx)) = parent_of_menu(&self.menu_root, sibling_id) else {
            return false;
        };
        if let Some(parent) = find_menu_mut(&mut self.menu_root, pid) {
            parent.children.insert(idx + 1, node);
            self.menu_selected = node_id;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn menu_remove_selected(&mut self) -> bool {
        if self.menu_selected == self.menu_root.id {
            return false;
        }
        let Some((pid, idx)) = parent_of_menu(&self.menu_root, self.menu_selected) else {
            return false;
        };
        let parent = find_menu_mut(&mut self.menu_root, pid).unwrap();
        parent.children.remove(idx);
        self.menu_selected = pid;
        self.dirty = true;
        true
    }

    pub fn menu_move_sibling(&mut self, delta: isize) -> bool {
        let Some((pid, idx)) = parent_of_menu(&self.menu_root, self.menu_selected) else {
            return false;
        };
        let parent = find_menu_mut(&mut self.menu_root, pid).unwrap();
        let n = parent.children.len();
        if n < 2 {
            return false;
        }
        let j = idx as isize + delta;
        if j < 0 || j >= n as isize {
            return false;
        }
        parent.children.swap(idx, j as usize);
        self.dirty = true;
        true
    }
}

fn walk_flat(node: &Node, depth: usize, out: &mut Vec<(usize, usize, String)>) {
    out.push((node.id, depth, node.tree_label()));
    for c in &node.children {
        walk_flat(c, depth + 1, out);
    }
}

fn find(node: &Node, id: usize) -> Option<&Node> {
    if node.id == id {
        return Some(node);
    }
    for c in &node.children {
        if let Some(n) = find(c, id) {
            return Some(n);
        }
    }
    None
}

fn find_mut(node: &mut Node, id: usize) -> Option<&mut Node> {
    if node.id == id {
        return Some(node);
    }
    for c in &mut node.children {
        if let Some(n) = find_mut(c, id) {
            return Some(n);
        }
    }
    None
}

/// Parent id and index of child `id` within that parent.
fn parent_of(root: &Node, id: usize) -> Option<(usize, usize)> {
    for (i, c) in root.children.iter().enumerate() {
        if c.id == id {
            return Some((root.id, i));
        }
        if let Some(p) = parent_of(c, id) {
            return Some(p);
        }
    }
    None
}

fn ancestor_kind(root: &Node, id: usize, kind: Kind) -> Option<usize> {
    let mut cur = id;
    loop {
        let (pid, _) = parent_of(root, cur)?;
        let p = find(root, pid)?;
        if p.kind == kind {
            return Some(p.id);
        }
        if pid == root.id {
            return if root.kind == kind {
                Some(root.id)
            } else {
                None
            };
        }
        cur = pid;
    }
}

fn walk_menu_flat(node: &MenuNode, depth: usize, out: &mut Vec<(usize, usize, String)>) {
    out.push((node.id, depth, node.tree_label()));
    for c in &node.children {
        walk_menu_flat(c, depth + 1, out);
    }
}

fn find_menu(node: &MenuNode, id: usize) -> Option<&MenuNode> {
    if node.id == id {
        return Some(node);
    }
    for c in &node.children {
        if let Some(n) = find_menu(c, id) {
            return Some(n);
        }
    }
    None
}

fn find_menu_mut(node: &mut MenuNode, id: usize) -> Option<&mut MenuNode> {
    if node.id == id {
        return Some(node);
    }
    for c in &mut node.children {
        if let Some(n) = find_menu_mut(c, id) {
            return Some(n);
        }
    }
    None
}

fn parent_of_menu(root: &MenuNode, id: usize) -> Option<(usize, usize)> {
    for (i, c) in root.children.iter().enumerate() {
        if c.id == id {
            return Some((root.id, i));
        }
        if let Some(p) = parent_of_menu(c, id) {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_reorder() {
        let mut d = Design::default();
        let root_id = d.root.id;
        d.selected = root_id;
        assert!(d.add_child(Kind::Input));
        assert!(d.add_child(Kind::List));
        assert_eq!(d.root.children.len(), 3); // Text + Input + List
        d.selected = d.root.children[2].id;
        assert!(d.move_sibling(-1));
    }

    #[test]
    fn add_tabs_wraps_a_pane() {
        let mut d = Design::default();
        d.selected = d.root.id;
        assert!(d.add_child(Kind::Tabs));
        let tabs = d.root.children.iter().find(|c| c.kind == Kind::Tabs).unwrap();
        assert_eq!(tabs.children.len(), 1);
        assert_eq!(tabs.children[0].kind, Kind::Tab);
        d.selected = tabs.children[0].id;
        assert!(d.add_child(Kind::Tab));
        let tabs = d.root.children.iter().find(|c| c.kind == Kind::Tabs).unwrap();
        assert_eq!(tabs.children.len(), 2);
        assert!(tabs.children.iter().all(|c| c.kind == Kind::Tab));
    }

    #[test]
    fn add_menu_item_creates_file() {
        let mut d = Design::default();
        d.menu_selected = d.menu_root.id;
        assert!(d.menu_add(MenuKind::Item));
        assert_eq!(d.menu_root.children.len(), 1);
        assert_eq!(d.menu_root.children[0].kind, MenuKind::Menu);
        assert_eq!(d.menu_root.children[0].children.len(), 1);
        assert_eq!(d.menu_root.children[0].children[0].kind, MenuKind::Item);
        assert!(d.menu_add(MenuKind::Menu));
        assert_eq!(d.menu_root.children.len(), 2);
    }
}
