(function() {
function el(p) {
  return {
    get tagName() {
      var t = __el_tag(p);
      return t === null ? null : t.toUpperCase();
    },
    get innerHTML() {
      return __el_html(p);
    },
    set innerHTML(v) {
      __el_set_html(p, String(v));
    },
    get textContent() {
      return __el_text(p);
    },
    set textContent(v) {
      __el_set_text(p, String(v));
    },
    get style() {
      return new Proxy({}, {
        get: function(t, n) {
          return __style_get(p, String(n));
        },
        set: function(t, n, v) {
          __style_set(p, String(n), String(v));
          return true;
        },
      });
    },
    getAttribute: function(n) {
      return __el_attr(p, String(n));
    },
    setAttribute: function(n, v) {
      __el_set_attr(p, String(n), String(v));
    },
  };
}
globalThis.document = {
  getElementById: function(id) {
    var p = __doc_by_id(String(id));
    return p === null ? null : el(p);
  },
  get body() {
    var p = __doc_body();
    return p === null ? null : el(p);
  },
};
})();
