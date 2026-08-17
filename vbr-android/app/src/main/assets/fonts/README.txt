Fonts TIDE can use (WebView @font-face). Not limited to Android's system set.

Drop a .ttf / .otf here and we can point --mono at it in editor.css, e.g.

  @font-face { font-family: "Mine"; src: url(fonts/whatever.ttf); }
  :root { --mono: "Mine", monospace; }

unscii-16.ttf — public domain (viznut / http://viznut.fi/unscii/). VGA-style; previous default.
