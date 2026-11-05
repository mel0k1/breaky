(function() {
globalThis.__els = [];
globalThis.__clicks = {};
globalThis.__listeners = {};
function inside(q, base) {
  return q === base || q.indexOf(base + ".") === 0;
}
function shiftKey(q, par, ix, up) {
  if (q === par || q.indexOf(par + ".") !== 0) return q;
  var rel = q.slice(par.length + 1);
  var dot = rel.indexOf(".");
  var head = dot === -1 ? rel : rel.slice(0, dot);
  var k = +head;
  if (isNaN(k)) return q;
  var hit = up ? k >= ix : k > ix;
  if (!hit) return q;
  var nk = up ? k + 1 : k - 1;
  return par + "." + nk + (dot === -1 ? "" : rel.slice(dot));
}
function remap(ops) {
  var flyW = [];
  var flyC = [];
  var flyL = [];
  for (var oi = 0; oi < ops.length; oi++) {
    var op = ops[oi];
    if (op.take !== undefined) {
      var from = op.take;
      var a = from.split(".");
      var ix = +a[a.length - 1];
      var par = a.slice(0, -1).join(".");
      var els = globalThis.__els;
      for (var i = 0; i < els.length; i++) {
        var q = els[i].__path;
        if (q === null) continue;
        if (inside(q, from)) {
          if (op.park !== undefined) {
            els[i].__path = op.park + q.slice(from.length);
          } else {
            flyW.push({ w: els[i], suffix: q.slice(from.length) });
            els[i].__path = null;
          }
          continue;
        }
        els[i].__path = shiftKey(q, par, ix, false);
      }
      globalThis.__clicks = retake(globalThis.__clicks, from, op.park, par, ix, flyC);
      globalThis.__listeners = retake(globalThis.__listeners, from, op.park, par, ix, flyL);
    } else if (op.put !== undefined) {
      var at = op.put.at;
      var pix = op.put.index;
      var els = globalThis.__els;
      for (var i = 0; i < els.length; i++) {
        var q = els[i].__path;
        if (q === null) {
          for (var f = 0; f < flyW.length; f++) {
            if (flyW[f].w === els[i]) els[i].__path = at + "." + pix + flyW[f].suffix;
          }
          continue;
        }
        els[i].__path = shiftKey(q, at, pix, true);
      }
      globalThis.__clicks = reput(globalThis.__clicks, at, pix, flyC);
      globalThis.__listeners = reput(globalThis.__listeners, at, pix, flyL);
    }
  }
}
function retake(map, from, parkKey, par, ix, fly) {
  var out = {};
  for (var k in map) {
    if (inside(k, from)) {
      if (parkKey !== undefined) {
        out[parkKey + k.slice(from.length)] = map[k];
      } else {
        fly.push({ key: k, suffix: k.slice(from.length), val: map[k] });
      }
      continue;
    }
    out[shiftKey(k, par, ix, false)] = map[k];
  }
  return out;
}
function reput(map, at, pix, fly) {
  var out = {};
  for (var k in map) out[shiftKey(k, at, pix, true)] = map[k];
  for (var i = 0; i < fly.length; i++) {
    if (fly[i].key !== undefined && fly[i].val !== undefined) {
      out[at + "." + pix + fly[i].suffix] = fly[i].val;
    }
  }
  return out;
}
function apply(raw) {
  if (raw === null) return false;
  remap(JSON.parse(raw).ops);
  return true;
}
function addLs(key, type, fn, capture) {
  if (typeof fn !== "function") return;
  var cap = capture === true;
  var ls = globalThis.__listeners[key];
  if (!ls) {
    ls = [];
    globalThis.__listeners[key] = ls;
  }
  for (var i = 0; i < ls.length; i++) {
    if (ls[i].type === type && ls[i].fn === fn && ls[i].capture === cap) return;
  }
  ls.push({ type: type, fn: fn, capture: cap });
}
function dropLs(key, type, fn, capture) {
  var cap = capture === true;
  var ls = globalThis.__listeners[key];
  if (!ls) return;
  for (var i = ls.length - 1; i >= 0; i--) {
    if (ls[i].type === type && ls[i].fn === fn && ls[i].capture === cap) ls.splice(i, 1);
  }
}
globalThis.__fire = function(type, targetPath, xs, ys, key) {
  var segs = targetPath === "" ? [] : targetPath.split(".");
  var chain = [];
  for (var s = segs.length; s >= 1; s--) chain.push(segs.slice(0, s).join("."));
  chain.push("");
  var tgt = targetPath === "" ? globalThis.document : el(targetPath);
  var ev = {
    type: type,
    target: tgt,
    currentTarget: null,
    clientX: parseFloat(xs),
    clientY: parseFloat(ys),
    key: key === undefined ? "" : String(key),
    button: 0,
    bubbles: true,
    defaultPrevented: false,
    preventDefault: function() {
      ev.defaultPrevented = true;
    },
    stopPropagation: function() {
      ev.__stop = true;
    },
    stopImmediatePropagation: function() {
      ev.__stop = true;
      ev.__stopNow = true;
    },
  };
  var fired = 0;
  var ci, i, ls;
  for (ci = chain.length - 1; ci >= 0 && !ev.__stop; ci--) {
    ev.currentTarget = chain[ci] === "" ? globalThis.document : el(chain[ci]);
    ls = globalThis.__listeners[chain[ci]];
    if (!ls) continue;
    for (i = 0; i < ls.length && !ev.__stopNow; i++) {
      if (ls[i].type === type && ls[i].capture) {
        ls[i].fn.call(ev.currentTarget, ev);
        fired++;
      }
    }
  }
  for (ci = 0; ci < chain.length && !ev.__stop; ci++) {
    ev.currentTarget = chain[ci] === "" ? globalThis.document : el(chain[ci]);
    ls = globalThis.__listeners[chain[ci]];
    if (ls) {
      for (i = 0; i < ls.length && !ev.__stopNow; i++) {
        if (ls[i].type === type && !ls[i].capture) {
          ls[i].fn.call(ev.currentTarget, ev);
          fired++;
        }
      }
    }
    if (type === "click" && globalThis.__clicks[chain[ci]]) {
      globalThis.__clicks[chain[ci]].call(ev.currentTarget, ev);
      fired++;
    }
  }
  return fired > 0;
};
function el(p) {
  var w = {
    __path: p,
    get tagName() {
      var t = __el_tag(this.__path);
      return t === null ? null : t.toUpperCase();
    },
    get innerHTML() {
      return __el_html(this.__path);
    },
    set innerHTML(v) {
      __el_set_html(this.__path, String(v));
    },
    get textContent() {
      return __el_text(this.__path);
    },
    set textContent(v) {
      __el_set_text(this.__path, String(v));
    },
    get style() {
      var self = this;
      return new Proxy({}, {
        get: function(t, n) {
          return __style_get(self.__path, String(n));
        },
        set: function(t, n, v) {
          __style_set(self.__path, String(n), String(v));
          return true;
        },
      });
    },
    getAttribute: function(n) {
      return __el_attr(this.__path, String(n));
    },
    setAttribute: function(n, v) {
      __el_set_attr(this.__path, String(n), String(v));
    },
    nodeType: 1,
    querySelector: function(sel) {
      var p = __el_query(this.__path, String(sel));
      return p === null ? null : el(p);
    },
    querySelectorAll: function(sel) {
      return toEls(__el_query_all(this.__path, String(sel)));
    },
    cloneNode: function(deep) {
      var p = __el_clone(this.__path, deep === true);
      return p === null ? null : el(p);
    },
    getBoundingClientRect: function() {
      var raw = __el_rect(this.__path);
      var r = raw === null ? { x: 0, y: 0, width: 0, height: 0 } : JSON.parse(raw);
      r.left = r.x;
      r.top = r.y;
      r.right = r.x + r.width;
      r.bottom = r.y + r.height;
      return r;
    },
    appendChild: function(c) {
      if (!c || typeof c.__path !== "string") return null;
      var cp = c.__path;
      var raw = __el_append(this.__path, cp);
      if (!apply(raw)) return null;
      return c;
    },
    insertBefore: function(c, ref) {
      if (!c || typeof c.__path !== "string") return null;
      var cp = c.__path;
      var rp = ref && typeof ref.__path === "string" ? ref.__path : "";
      var raw = __el_insert(this.__path, cp, rp);
      if (!apply(raw)) return null;
      return c;
    },
    removeChild: function(c) {
      if (!c || typeof c.__path !== "string") return null;
      var cp = c.__path;
      var raw = __el_remove(this.__path, cp);
      if (!apply(raw)) return null;
      return c;
    },
    addEventListener: function(type, fn, capture) {
      addLs(this.__path, type, fn, capture);
    },
    removeEventListener: function(type, fn, capture) {
      dropLs(this.__path, type, fn, capture);
    },
    get onclick() {
      return globalThis.__clicks[this.__path] || null;
    },
    set onclick(fn) {
      if (typeof fn === "function") {
        globalThis.__clicks[this.__path] = fn;
      } else {
        delete globalThis.__clicks[this.__path];
      }
    },
  };
  globalThis.__els.push(w);
  return w;
}
function toEls(raw) {
  var ps = JSON.parse(raw);
  var out = [];
  for (var i = 0; i < ps.length; i++) out.push(el(ps[i]));
  return out;
}
function txt(p) {
  var w = {
    __path: p,
    nodeType: 3,
    get data() {
      return __text_data(this.__path);
    },
    set data(v) {
      __text_set_data(this.__path, String(v));
    },
    get textContent() {
      return __text_data(this.__path);
    },
    set textContent(v) {
      __text_set_data(this.__path, String(v));
    },
    cloneNode: function() {
      var np = __el_clone(this.__path, true);
      return np === null ? null : txt(np);
    },
  };
  globalThis.__els.push(w);
  return w;
}
globalThis.document = {
  getElementById: function(id) {
    var p = __doc_by_id(String(id));
    return p === null ? null : el(p);
  },
  createElement: function(tag) {
    var p = __doc_create(String(tag));
    return p === null ? null : el(p);
  },
  createTextNode: function(data) {
    var p = __doc_create_text(String(data));
    return p === null ? null : txt(p);
  },
  querySelector: function(sel) {
    var p = __doc_query(String(sel));
    return p === null ? null : el(p);
  },
  querySelectorAll: function(sel) {
    return toEls(__doc_query_all(String(sel)));
  },
  addEventListener: function(type, fn, capture) {
    addLs("", type, fn, capture);
  },
  removeEventListener: function(type, fn, capture) {
    dropLs("", type, fn, capture);
  },
  get body() {
    var p = __doc_body();
    return p === null ? null : el(p);
  },
};
})();
