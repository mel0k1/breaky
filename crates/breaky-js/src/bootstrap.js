(function() {
function el(p) {
  return {
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
    appendChild: function(c) {
      var cp = c && c.__path ? c.__path : "";
      var np = __el_append(this.__path, cp, c || null);
      return np === null ? null : el(np);
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
}
globalThis.__clicks = {};
globalThis.document = {
  getElementById: function(id) {
    var p = __doc_by_id(String(id));
    return p === null ? null : el(p);
  },
  createElement: function(tag) {
    var p = __doc_create(String(tag));
    return p === null ? null : el(p);
  },
  querySelector: function(sel) {
    var p = __doc_query(String(sel));
    return p === null ? null : el(p);
  },
  get body() {
    var p = __doc_body();
    return p === null ? null : el(p);
  },
};
})();
