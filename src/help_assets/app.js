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

  function paras(s) {
    return s.split(/\n\n+/).map(function (p) { return '<p>' + esc(p) + '</p>'; }).join('');
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

  function renderEntry(e) {
    if (!e) {
      contentEl.innerHTML = '<div class="welcome"><h1>VBR Help</h1>'
        + '<p>Pick a topic on the left, or press <kbd>/</kbd> to search.</p>'
        + '<p>Every example is real code that transpiles — the generated Rust is shown beneath it.</p></div>';
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

  function route() {
    renderEntry(byAnchor[location.hash.slice(1).toLowerCase()]);
    renderList(searchEl.value);
  }

  contentEl.addEventListener('click', function (ev) {
    var btn = ev.target;
    if (!btn.classList || !btn.classList.contains('copy')) return;
    var pre = btn.parentNode.querySelector('pre');
    if (pre) copyText(pre.textContent, btn);
  });

  searchEl.addEventListener('input', function () { renderList(searchEl.value); });
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
    var MIN = 180, MAX = 560;
    try {
      var saved = parseInt(localStorage.getItem('vbr_help_navw'), 10);
      if (saved >= MIN && saved <= MAX) nav.style.width = saved + 'px';
    } catch (e) {}
    function onMove(ev) {
      var w = Math.max(MIN, Math.min(MAX, ev.clientX - nav.getBoundingClientRect().left));
      nav.style.width = w + 'px';
    }
    function onUp() {
      document.body.classList.remove('dragging');
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      try { localStorage.setItem('vbr_help_navw', parseInt(nav.style.width, 10)); } catch (e) {}
    }
    drag.addEventListener('mousedown', function (ev) {
      ev.preventDefault();
      document.body.classList.add('dragging');
      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    });
  })();

  route();
})();
