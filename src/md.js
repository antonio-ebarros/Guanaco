// Renderizador de Markdown mínimo e offline. Expõe window.md(texto, cursor?)
window.md = (() => {
  const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  const KW = new Set(('if else elif for while do return function def class import from as const let var int char float double void struct enum static public private protected new try except catch finally with in not and or is switch case break continue async await fn pub use mod impl typeof package func go range defer bool string long unsigned sizeof lambda yield raise pass echo then fi done match type interface typealias super select where insert update delete into values create table drop alter join on group by order limit').split(' '));
  const LITERALS = new Set(['True', 'False', 'None', 'null', 'undefined', 'true', 'false', 'nil', 'self', 'this']);
  const HASH_LANGS = new Set(['python', 'py', 'bash', 'sh', 'shell', 'zsh', 'yaml', 'yml', 'toml', 'ruby', 'rb', 'dockerfile', 'makefile', 'conf', 'ini', 'r']);
  const SQL_LANGS = new Set(['sql', 'sqlite', 'psql', 'mysql']);
  const HTML_LANGS = new Set(['html', 'xml', 'svg', 'vue']);
  const PLAIN = new Set(['', 'text', 'txt', 'plaintext', 'output', 'console']);

  function highlight(code, lang) {
    lang = (lang || '').toLowerCase();
    if (PLAIN.has(lang)) return esc(code);

    let commentRe = '\\/\\/.*|\\/\\*[\\s\\S]*?\\*\\/';
    if (HASH_LANGS.has(lang)) commentRe = '#.*';
    else if (SQL_LANGS.has(lang)) commentRe = '--.*|\\/\\*[\\s\\S]*?\\*\\/';
    else if (HTML_LANGS.has(lang)) commentRe = '&lt;!--[\\s\\S]*?--&gt;|<!--[\\s\\S]*?-->';

    const strRe = '[frbFRB]?\"\"\"[\\s\\S]*?\"\"\"|[frbFRB]?\'\'\'[\\s\\S]*?\'\'\'|`(?:\\\\.|[^`\\\\])*`|[frbFRB]?\"(?:\\\\.|[^\"\\\\\\n])*\"|[frbFRB]?\'(?:\\\\.|[^\'\\\\\\n])*\'';
    const numRe = '\\b(?:0x[0-9a-fA-F]+|0b[01]+|\\d+(?:\\.\\d+)?)\\b';
    const decRe = '@[A-Za-z_]\\w*';
    const wordRe = '\\b[A-Za-z_]\\w*\\b';
    const opRe = '==|!=|<=|>=|\\+=|-=|\\*=|/=|%=|=>|->|[+\\-*\\/%&|^~<>!=]';

    const pattern = `(${commentRe})|(${strRe})|(${numRe})|(${decRe})|(${wordRe})|(${opRe})`;
    const re = new RegExp(pattern, 'g');

    let out = '', last = 0, m;
    while ((m = re.exec(code))) {
      out += esc(code.slice(last, m.index));
      const [_, c, s, n, d, w, op] = m;
      let cls = '';
      if (c) cls = 't-c';
      else if (s) cls = 't-s';
      else if (n) cls = 't-n';
      else if (d) cls = 't-d';
      else if (w) {
        const nextChar = code[m.index + m[0].length];
        if (LITERALS.has(w)) cls = 't-l';
        else if (KW.has(w.toLowerCase())) cls = 't-k';
        else if (nextChar === '(') cls = 't-f';
        else {
          const prev = code.slice(0, m.index).trim();
          if (/(?:def|class|fn|function|struct|interface)\s+$/.test(prev)) cls = 't-def';
        }
      } else if (op) {
        cls = 't-op';
      }
      out += cls ? `<span class="${cls}">${esc(m[0])}</span>` : esc(m[0]);
      last = re.lastIndex;
      if (m[0] === '') re.lastIndex++;
    }
    return out + esc(code.slice(last));
  }

  function inline(s) {
    const codes = [];
    s = esc(s).replace(/`([^`\n]+)`/g, (_, c) => { codes.push(c); return `\u0000${codes.length - 1}\u0000`; });
    s = s
      .replace(/\[([^\]\n]+)\]\((https?:\/\/[^\s)]+|mailto:[^\s)]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>')
      .replace(/\*\*([^*\n]+)\*\*|__([^_\n]+)__/g, (_, a, b) => `<b>${a || b}</b>`)
      .replace(/(^|[^*\w])\*([^*\s][^*\n]*?)\*(?![*\w])/g, '$1<i>$2</i>')
      .replace(/(^|[^\w])_([^_\s][^_\n]*?)_(?!\w)/g, '$1<i>$2</i>')
      .replace(/~~([^~\n]+)~~/g, '<s>$1</s>');
    return s.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${codes[i]}</code>`);
  }

  const LI = /^(\s*)([-*+]|\d+[.)])\s+(.*)/;
  const HR = /^\s*([-*_])(\s*\1){2,}\s*$/;
  const SEP = /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/;
  const special = l => /^```/.test(l) || /^#{1,4}\s/.test(l) || /^>/.test(l) || LI.test(l) || HR.test(l);
  const cells = l => l.trim().replace(/^\||\|$/g, '').split('|').map(c => inline(c.trim()));

  function codeBlock(code, lang) {
    const raw = code.replace(/^\n+|\n+$/g, '');
    const lines = raw.split('\n');
    const lineNumbers = lines.length > 1
      ? `<div class="code-lines" aria-hidden="true">${lines.map((_, i) => `<span>${i + 1}</span>`).join('')}</div>`
      : '';
    const label = esc((lang || 'código').toUpperCase());
    return `<div class="code" data-lang="${esc(lang || '')}"><div class="code-bar"><span class="code-lang"><span class="code-dots"><i></i><i></i><i></i></span>${label}</span><button type="button" class="copy" title="Copiar código">📋 Copiar</button></div><div class="code-body">${lineNumbers}<pre><code>${highlight(raw, lang)}</code></pre></div></div>`;
  }

  function renderList(items) {
    let h = '', st = [];
    for (const it of items) {
      while (st.length && it.d < st[st.length - 1].d) h += `</li></${st.pop().tag}>`;
      const top = st[st.length - 1];
      if (!top || it.d > top.d) {
        const tag = it.o ? 'ol' : 'ul';
        h += `<${tag}${it.o && it.n !== 1 ? ` start="${it.n}"` : ''}><li>`;
        st.push({ d: it.d, tag });
      } else h += '</li><li>';
      h += inline(it.t);
    }
    while (st.length) h += `</li></${st.pop().tag}>`;
    return h;
  }

  function blocks(src) {
    const L = src.replace(/\r/g, '').split('\n'), out = [];
    let i = 0, m;
    while (i < L.length) {
      const l = L[i];
      if (!l.trim()) { i++; continue; }
      if ((m = l.match(/^```\s*([\w+-]*)/))) {
        const buf = []; i++;
        while (i < L.length && !/^```/.test(L[i])) buf.push(L[i++]);
        i++; out.push(codeBlock(buf.join('\n'), m[1])); continue;
      }
      if (HR.test(l)) { out.push('<hr>'); i++; continue; }
      if ((m = l.match(/^(#{1,4})\s+(.*)/))) { out.push(`<h${m[1].length}>${inline(m[2])}</h${m[1].length}>`); i++; continue; }
      if (l.includes('|') && L[i + 1] !== undefined && SEP.test(L[i + 1])) {
        const head = cells(l); i += 2; const rows = [];
        while (i < L.length && L[i].includes('|') && L[i].trim()) rows.push(cells(L[i++]));
        out.push('<div class="tbl"><table><thead><tr>' + head.map(c => `<th>${c}</th>`).join('') + '</tr></thead><tbody>' +
          rows.map(r => '<tr>' + r.map(c => `<td>${c}</td>`).join('') + '</tr>').join('') + '</tbody></table></div>');
        continue;
      }
      if (/^>/.test(l)) {
        const buf = [];
        while (i < L.length && /^>/.test(L[i])) buf.push(L[i++].replace(/^>\s?/, ''));
        out.push(`<blockquote>${blocks(buf.join('\n'))}</blockquote>`); continue;
      }
      if (LI.test(l)) {
        const items = [];
        while (i < L.length) {
          const mm = L[i].match(LI);
          if (mm) { items.push({ d: mm[1].replace(/\t/g, '  ').length, o: /\d/.test(mm[2]), n: parseInt(mm[2]), t: mm[3] }); i++; }
          else if (!L[i].trim() && L[i + 1] !== undefined && LI.test(L[i + 1])) i++;
          else if (/^\s+\S/.test(L[i]) && items.length && !special(L[i])) { items[items.length - 1].t += ' ' + L[i].trim(); i++; }
          else break;
        }
        out.push(renderList(items)); continue;
      }
      const buf = [l]; i++;
      while (i < L.length && L[i].trim() && !special(L[i]) && !(L[i].includes('|') && L[i + 1] !== undefined && SEP.test(L[i + 1]))) buf.push(L[i++]);
      out.push(`<p>${buf.map(inline).join('<br>')}</p>`);
    }
    return out.join('');
  }

  return (src, cursor) => {
    let html = blocks(src || '');
    if (cursor) {
      const C = '<span class="cursor"></span>';
      const re = /(<\/(?:p|code|li)>)((?:<\/(?:li|ul|ol|pre|div|blockquote)>)*)$/;
      html = re.test(html) ? html.replace(re, C + '$1$2') : html + C;
    }
    return html;
  };
})();
window.renderMarkdown = window.md;
