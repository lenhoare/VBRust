
Yes. Looking at this with your current TUI model in mind, I think there are four historical design traditions, and the best Bust designer is probably a hybrid of two of them.

The first is the classic resource/dialog editor: empty form, palette of controls, place them, resize them, edit properties, then test. Borland Resource Workshop worked this way, and modern descendants still do. It solved the problem, but I agree with your instinct: mouse positioning becomes fiddly and starts making the designer itself the work. 

The second is much more interesting for Bust: structural form design. You edit the hierarchy and properties rather than drawing pixels. Clarion's designers were strongly oriented around controls plus properties, and Turbo Vision had third-party interactive dialog designers that generated actual Pascal/C++ dialog source. 

Your existing TUI language is unusually well suited to this because the screen already is a tree:

Screen
└─ Column
   ├─ Text
   ├─ Row
   │  ├─ List
   │  └─ Column
   │     ├─ Text
   │     └─ Input
   └─ Text

And importantly, you're not really positioning widgets at (x,y) anyway. You're composing Row, Column, Length, Fill, Percent, Padding, etc. That removes much of the justification for a conventional visual designer.

So I would make the sibling designer primarily a structural screen composer.

Something like:

┌─ Screen Designer ───────────────────────────────────────┐
│                                                        │
│  Components                  Preview                    │
│  ──────────                  ────────────────────────   │
│  > Column                    │ Customers             │   │
│    Text                      │                       │   │
│    Row                       │ Alice    £1200        │   │
│      List customers          │ Bob       £750        │   │
│      Column                  │ Charlie  £2300        │   │
│        Text                  │                       │   │
│        Input search          │ Search: ________      │   │
│                                                        │
├────────────────────────────────────────────────────────┤
│ F2 Add   Del Remove   Alt+↑↓ Move   Enter Properties   │
│ F9 Preview   F10 Code                                  │
└────────────────────────────────────────────────────────┘

The killer operation: Add

Cursor sits on a node. Hit perhaps F2 or Ctrl+Space:

Add component
─────────────
Text
Input
List
Table
Row
Column
Gauge
Chart
If
Match

Type inp, Enter:

Input
Field: [search________]
Submit event: [Search______]
Size: [Default ▼]

Enter again and it's inserted at the structurally sensible location.

That is probably considerably faster than grabbing an Input from a toolbox and dragging it somewhere.

I'd make movement structural too:

Alt+Up/Down — reorder among siblings

Alt+Left — move out one level

Alt+Right — nest inside preceding compatible container

Ctrl+D — duplicate

Del — remove

Enter — properties

Ctrl+Space — insert


Those six operations could build screens extremely quickly.

The preview should be live essentially all the time. You're in a TUI already, so rendering the real Ratatui representation into the right-hand rectangle should be cheap. F9 could then switch from designer preview into interactive preview, where Tab/Input/List/etc actually behave. That mirrors the old dialog-editor idea of having a test mode without requiring a full compile/run cycle. 

I would also make the preview selectable, but not primarily mouse editable. For example, moving around the structural tree highlights the corresponding area:

> Input search

and the preview shows:

┌─────────────────────┐
│ Search: ███████████ │   ← highlighted
└─────────────────────┘

Clicking something in the preview could select the corresponding tree node. That's useful. Dragging it around probably isn't.

Another historical idea worth stealing is the object inspector/property editor, but keep yours tiny. Press Enter on:

List customers

and get:

List
────────────────────
Field       customers
On Select   OpenCustomer
Size        Fill

Most changes become one or two keystrokes.

There is also a third approach that I think would suit you extremely well: templates as first-class components.

Hit F2:

Layouts
──────────────
Header / Body / Footer
Master / Detail
Search / Results
Form
Dashboard
Two Column
Tabbed Screen
Dialog

Choosing Master / Detail could immediately create:

Row
    Percent 35
    List items
    Fill
    Column
        Text title
        Text details
    End Column
End Row

That's much closer to the actual thing you want: speeding up screen creation, rather than meticulously designing one.

And I'd go one step further: allow users eventually to save any subtree as a template.

Save Selection as Component...

Then:

My Components
─────────────
Customer Picker
Status Footer
Search Panel
Error Dialog

That becomes surprisingly powerful without introducing a component framework into the language itself. It's simply source-tree generation.

One thing I would not do is invent a separate .form or designer-resource format if you can avoid it. Borland/Windows historically needed resources because GUI descriptions and source code were fairly different worlds. Your Bust View syntax is already a nice declarative screen description. The designer should preferably edit the actual Bust AST/source. Turbo Vision dialog designers that generated source are a useful precedent here. 

So you get this lovely relationship:

CODE EDITOR                    SCREEN DESIGNER

View                           Column
    Column                       Text title
        Text title       ⇄       Row
        Row                        List users
            List users             Column
            Column                   Input search
                Input search
            End Column
        End Row
    End Column
End View

There isn't really a "designer file". They're simply two views onto the same program.

And that leads to what I think is the particularly nice Bust idea:

F10 toggles Code / Designer for the current Screen, exactly like old Delphi-style Code/Form switching, but both sides remain textual/TUI experiences. Modern RAD Studio still explicitly exposes this Code/Design relationship. 

I think the resulting philosophy should be:

> Don't draw the screen. Build its structure quickly and see it continuously.



For your language, that is arguably better than recreating Delphi in text mode. The fundamental representation of a TUI is already hierarchical layout, so the designer ought to expose that hierarchy directly.

I can already imagine it being exceptionally pleasant: arrow around the tree, Ctrl+Space, type tab, Enter, type the collection name, Enter — and there's a working table in the preview. That feels very much in keeping with what you've built rather than bolting a GUI-era form designer onto it.
