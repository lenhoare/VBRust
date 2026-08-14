// GENERATED into help/build/site/ by `vbr help build` — do not edit there.
// The offline help browser: client-side search + hash routing (#kw/For …),
// reading everything from the generated data.js. No server, no fetch.
(function () {
  var H = window.VBR_HELP || { entries: [] };
  var entries = H.entries;
  var topics = H.topics || {};
  var byId = {}, byAnchor = {};
  entries.forEach(function (e) {
    byId[e.id] = e;
    byAnchor[e.anchor.toLowerCase()] = e;
  });

  var listEl = document.getElementById('list');
  var contentEl = document.getElementById('content');
  var searchEl = document.getElementById('search');
  var countEl = document.getElementById('count');

  function esc(s) {
    return (s || '').replace(/[&<>]/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c];
    });
  }

  function matches(e, q) {
    if (!q) return true;
    q = q.toLowerCase();
    return e.title.toLowerCase().indexOf(q) >= 0
      || e.id.toLowerCase().indexOf(q) >= 0
      || (e.summary || '').toLowerCase().indexOf(q) >= 0
      || (e.category || '').toLowerCase().indexOf(q) >= 0;
  }

  function renderList(q) {
    var groups = {}, order = [], n = 0;
    var curAnchor = location.hash.slice(1).toLowerCase();
    entries.forEach(function (e) {
      if (e.parent && !q) return;   // member pages appear only when searching
      if (!matches(e, q)) return;
      n++;
      if (!groups[e.category]) { groups[e.category] = []; order.push(e.category); }
      groups[e.category].push(e);
    });
    var html = '';
    order.forEach(function (cat) {
      html += '<div class="cat">' + esc(cat) + '</div>';
      groups[cat].forEach(function (e) {
        var cur = curAnchor === e.anchor.toLowerCase() ? ' cur' : '';
        html += '<a class="item' + cur + '" href="#' + e.anchor + '">'
          + '<span class="k k-' + esc(e.kind) + '">' + esc(e.kind.charAt(0).toUpperCase()) + '</span>'
          + esc(e.title) + '</a>';
      });
    });
    listEl.innerHTML = html || '<div class="empty">No matches.</div>';
    countEl.textContent = n + ' / ' + entries.length;
  }

  // Escape HTML, then promote `code` spans (backticks) to <code> — so remarks
  // and cautions render inline code the way the rest of the reference does.
  function inl(s) {
    return esc(s).replace(/`([^`]+)`/g, '<code>$1</code>');
  }
  function paras(s) {
    return s.split(/\n\n+/).map(function (p) { return '<p>' + inl(p) + '</p>'; }).join('');
  }

  // The member id a signature points at, e.g. parent "vec" + ".Push(item)" -> "vec.push".
  function memberId(parentId, sig) {
    var name = sig.replace(/^\./, '').split('(')[0].trim().toLowerCase();
    return parentId + '.' + name;
  }

  // A VB-style Properties / Methods table. Each row links to its own page when
  // one exists (degrades to plain text otherwise, like a See-also stub).
  function members(heading, list, parentId) {
    if (!list || !list.length) return '';
    var rows = list.map(function (m) {
      var page = byId[memberId(parentId, m.sig)];
      var sig = page
        ? '<a href="#' + page.anchor + '"><code>' + esc(m.sig) + '</code></a>'
        : '<code>' + esc(m.sig) + '</code>';
      return '<tr><td class="msig">' + sig + '</td>'
        + '<td class="mdesc">' + esc(m.desc) + '</td></tr>';
    }).join('');
    return '<h2>' + heading + '</h2><table class="members">' + rows + '</table>';
  }

  // A code block with a hover copy button (delegated click below). `html` is
  // already syntax-highlighted at build time; the copy button reads back the
  // rendered text, so no separate plain copy is needed.
  function code(cls, html) {
    return '<div class="code"><button type="button" class="copy" title="Copy">Copy</button>'
      + '<pre class="' + cls + '"><code>' + html + '</code></pre></div>';
  }

  function copyText(text, btn) {
    var done = function () {
      var was = btn.textContent;
      btn.textContent = 'Copied'; btn.classList.add('ok');
      setTimeout(function () { btn.textContent = was; btn.classList.remove('ok'); }, 1200);
    };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(done, function () { fallbackCopy(text); done(); });
    } else {
      fallbackCopy(text); done();
    }
  }
  function fallbackCopy(text) {
    var ta = document.createElement('textarea');
    ta.value = text; ta.style.position = 'fixed'; ta.style.opacity = '0';
    document.body.appendChild(ta); ta.select();
    try { document.execCommand('copy'); } catch (e) {}
    document.body.removeChild(ta);
  }

  var WELCOME =
    '<div class="welcome">'
    + '<h1>VBR Help</h1>'
    + '<p class="summary">VBR is a modern dialect of Visual Basic that compiles to clean, '
    +   'idiomatic Rust. You write in the familiar <code>Sub</code>/<code>Function</code>, '
    +   '<code>Dim … As</code>, <code>If … Then</code> shape of VB6 and VBA — and out comes '
    +   'real Rust you can read, run, and learn from.</p>'
    + '<p>It is built as a teaching bridge: every keyword, type and example in this reference '
    +   'shows the VB you write alongside the Rust it becomes. Nothing here is a mock-up — each '
    +   'example is compiled, and the generated Rust is printed beneath it.</p>'
    + '<p>Pick a topic on the left (tap <b>☰</b> on a phone), or press <kbd>/</kbd> to search.</p>'

    + '<h2>How VBR differs from VB</h2>'
    + '<p>If you know VB6 or VBA most of this will feel like home. A handful of things are '
    +   'deliberately different — usually because VBR leans on Rust’s type system instead of '
    +   'the old runtime.</p>'
    + '<table class="members"><tbody>'
    + row('Static types, no <code>Variant</code>',
          'Every <code>Dim</code> has a real type the compiler checks. Alongside the VB types '
        + 'you get Rust ones — <code>Vec&lt;T&gt;</code>, <code>HashMap&lt;K, V&gt;</code>, '
        + '<code>Option&lt;T&gt;</code>, <code>Result&lt;T&gt;</code>.')
    + row('<code>Match … End Match</code>',
          'Replaces <code>Select Case</code>. Arms are <code>pattern =&gt; body</code> and can '
        + 'match on the <i>shape</i> of data, not just equality.')
    + row('<code>Is</code> binds patterns',
          '<code>If total Is Some(v) Then …</code> unwraps an <code>Option</code>/'
        + '<code>Result</code> inline — VB’s <code>Is</code> now does Rust’s <i>if-let</i>.')
    + row('Errors are values',
          'Fallible work returns a <code>Result</code> you handle explicitly (<code>.Unwrap</code>, '
        + 'match, or propagate) — there is no <code>On Error GoTo</code>.')
    + row('Methods keep their Rust names',
          'A method <i>is</i> its Rust name: <code>Is_Empty</code>, <code>Unwrap_Or</code>, '
        + '<code>Contains_Key</code>. Letters are case-insensitive; the underscores are literal.')
    + row('Enums carry data',
          '<code>Enum</code> defines sum types like Rust’s — a case can hold values, not just a '
        + 'number — so you can model shapes VB’s constant enums never could.')
    + row('Strings &amp; text',
          'Quotes are still doubled VB-style (<code>""</code>), with no backslash escapes; '
        + '<code>Text … End Text</code> holds multi-line blocks verbatim.')
    + row('A batteries-included stdlib',
          'Namespaces cover real work — <code>FileSystem</code>, <code>Http</code>, '
        + '<code>Json</code>, <code>DateTime</code>, <code>Regex</code>, <code>Database</code>, '
        + '<code>DataFrame</code>, <code>Shell</code>.')
    + '</tbody></table>'
    + '</div>';

  function row(sig, desc) {
    return '<tr><td class="msig">' + sig + '</td><td class="mdesc">' + desc + '</td></tr>';
  }

  function renderEntry(e) {
    if (!e) {
      contentEl.innerHTML = WELCOME;
      return;
    }
    var h = '';
    if (e.parent && byId[e.parent]) {
      var p = byId[e.parent];
      h += '<div class="crumb"><a href="#' + p.anchor + '">' + esc(p.title) + '</a> &rsaquo; '
        + '<span class="anchor">#' + esc(e.anchor) + '</span></div>';
    } else {
      h += '<div class="crumb">' + esc(e.category) + ' &middot; <span class="anchor">#' + esc(e.anchor) + '</span></div>';
    }
    h += '<h1>' + esc(e.title) + ' <span class="k k-' + esc(e.kind) + '">' + esc(e.kind) + '</span></h1>';
    if (e.summary) h += '<p class="summary">' + esc(e.summary) + '</p>';
    if (e.has_syntax) h += '<h2>Syntax</h2>' + code('syntax', e.syntax_html);
    if (e.cautions && e.cautions.length) {
      e.cautions.forEach(function (c) {
        h += '<div class="caution"><div class="caution-head">'
          + '<span class="caution-tag">⚠ Caution</span> ' + esc(c.summary) + '</div>'
          + '<div class="caution-body">' + inl(c.body) + '</div></div>';
      });
    }
    if (e.replaces) {
      h += '<div class="replaces"><span class="rep-tag">⇄ Replaces</span>'
        + '<span class="rep-body">VB\'s <b>' + esc(e.replaces) + '</b></span></div>';
    }
    if (e.arguments && e.arguments.length) {
      h += '<h2>Arguments</h2><table class="members">';
      e.arguments.forEach(function (a) {
        h += '<tr><td class="msig"><code>' + esc(a.name) + '</code> <span class="argty">'
          + esc(a.ty) + '</span></td><td class="mdesc">' + esc(a.desc) + '</td></tr>';
      });
      h += '</table>';
    }
    if (e.returns) h += '<h2>Returns</h2><p class="returns">' + esc(e.returns) + '</p>';
    if (e.remarks) h += '<h2>Remarks</h2>' + paras(e.remarks);
    h += members('Properties', e.properties, e.id);
    h += members('Methods', e.methods, e.id);
    if (e.has_example) h += '<h2>Example</h2>' + code('vb', e.example_html);
    if (e.has_rust) h += '<h2>Generated Rust</h2>' + code('rust', e.rust_html);
    if (e.see_also && e.see_also.length) {
      h += '<h2>See also</h2><div class="see">';
      e.see_also.forEach(function (id) {
        var meta = topics[id] || byId[id];
        var title = meta ? meta.title : id;
        if (byId[id]) {
          h += '<a href="#' + byId[id].anchor + '">' + esc(title) + '</a>';
        } else {
          h += '<span class="see-stub" title="Not documented yet">' + esc(title) + '</span>';
        }
      });
      h += '</div>';
    }
    contentEl.innerHTML = h;
    contentEl.scrollTop = 0;
  }

  function isMobile() { return window.innerWidth <= 820; }

  function route() {
    var e = byAnchor[location.hash.slice(1).toLowerCase()];
    renderEntry(e);
    renderList(searchEl.value);
    // On a phone the list is a drawer that closes once you've chosen —
    // you read the article full-screen; the ☰ button reopens the list.
    if (isMobile()) document.body.classList.add('nav-collapsed');
  }

  var brand = document.getElementById('brand');
  if (brand) {
    brand.addEventListener('click', function (ev) {
      ev.preventDefault();
      searchEl.value = '';
      if (location.hash) location.hash = '';   // triggers route() via hashchange
      else route();                            // already home — just re-render
      if (isMobile()) document.body.classList.add('nav-collapsed');
    });
  }

  var navToggle = document.getElementById('navtoggle');
  if (navToggle) {
    navToggle.addEventListener('click', function () {
      document.body.classList.toggle('nav-collapsed');
    });
  }

  contentEl.addEventListener('click', function (ev) {
    var btn = ev.target;
    if (!btn.classList || !btn.classList.contains('copy')) return;
    var pre = btn.parentNode.querySelector('pre');
    if (pre) copyText(pre.textContent, btn);
  });

  searchEl.addEventListener('input', function () {
    // Typing a query means "show me the list" — reveal it on mobile.
    if (isMobile()) document.body.classList.remove('nav-collapsed');
    renderList(searchEl.value);
  });
  searchEl.addEventListener('keydown', function (ev) {
    if (ev.key !== 'Enter') return;
    // Enter jumps straight to the first match (sidebar order).
    var q = searchEl.value;
    for (var i = 0; i < entries.length; i++) {
      if (matches(entries[i], q)) { location.hash = '#' + entries[i].anchor; searchEl.blur(); break; }
    }
  });
  window.addEventListener('hashchange', route);
  document.addEventListener('keydown', function (ev) {
    if (ev.key === '/' && document.activeElement !== searchEl) {
      ev.preventDefault(); searchEl.focus(); searchEl.select();
    } else if (ev.key === 'Escape' && document.activeElement === searchEl) {
      searchEl.blur();
    }
  });

  // Draggable divider between the sidebar and the content pane.
  (function () {
    var nav = listEl, drag = document.getElementById('drag');
    if (!drag) return;
    var MIN = 96, MAX = 560;
    // On a phone the sidebar defaults to a third of the width (CSS); only
    // restore a pixel width the user chose on a wide screen.
    try {
      var saved = parseInt(localStorage.getItem('vbr_help_navw'), 10);
      if (window.innerWidth > 640 && saved >= MIN && saved <= MAX) nav.style.width = saved + 'px';
    } catch (e) {}
    function onMove(ev) {
      var w = Math.max(MIN, Math.min(MAX, ev.clientX - nav.getBoundingClientRect().left));
      nav.style.width = w + 'px';
    }
    function onUp(ev) {
      document.body.classList.remove('dragging');
      document.removeEventListener('pointermove', onMove);
      document.removeEventListener('pointerup', onUp);
      try { drag.releasePointerCapture(ev.pointerId); } catch (e) {}
      try { localStorage.setItem('vbr_help_navw', parseInt(nav.style.width, 10)); } catch (e) {}
    }
    // Pointer Events cover mouse, touch and pen with one path.
    drag.addEventListener('pointerdown', function (ev) {
      ev.preventDefault();
      try { drag.setPointerCapture(ev.pointerId); } catch (e) {}
      document.body.classList.add('dragging');
      document.addEventListener('pointermove', onMove);
      document.addEventListener('pointerup', onUp);
    });
  })();

  route();
})();
