mod dom;

use html::Node;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

#[repr(C)]
#[derive(Clone, Copy)]
struct JSValue {
    u: JSValueUnion,
    tag: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
union JSValueUnion {
    as_int32: i32,
    as_float64: f64,
    as_ptr: *mut c_void,
}

#[repr(C)]
struct JSRuntime {
    _p: [u8; 0],
}

#[repr(C)]
struct JSContext {
    _p: [u8; 0],
}

type JSCFunction =
    unsafe extern "C" fn(*mut JSContext, JSValue, c_int, *const JSValue) -> JSValue;

extern "C" {
    fn JS_NewRuntime() -> *mut JSRuntime;
    fn JS_NewContext(rt: *mut JSRuntime) -> *mut JSContext;
    fn JS_FreeRuntime(rt: *mut JSRuntime);
    fn JS_FreeContext(ctx: *mut JSContext);
    fn JS_NewCFunction2(
        ctx: *mut JSContext,
        f: JSCFunction,
        name: *const c_char,
        length: c_int,
        cproto: c_int,
        magic: c_int,
    ) -> JSValue;
    fn JS_GetGlobalObject(ctx: *mut JSContext) -> JSValue;
    fn JS_NewStringLen(ctx: *mut JSContext, s: *const c_char, len: usize) -> JSValue;
    fn JS_SetPropertyStr(
        ctx: *mut JSContext,
        obj: JSValue,
        prop: *const c_char,
        val: JSValue,
    ) -> c_int;
    fn JS_GetPropertyStr(ctx: *mut JSContext, obj: JSValue, prop: *const c_char) -> JSValue;
    fn JS_GetContextOpaque(ctx: *mut JSContext) -> *mut c_void;
    fn JS_SetContextOpaque(ctx: *mut JSContext, opaque: *mut c_void);
    fn JS_Call(
        ctx: *mut JSContext,
        f: JSValue,
        this: JSValue,
        argc: c_int,
        argv: *const JSValue,
    ) -> JSValue;
    fn JS_Eval(
        ctx: *mut JSContext,
        input: *const c_char,
        len: usize,
        filename: *const c_char,
        flags: c_int,
    ) -> JSValue;
    fn breaky_is_exception(v: JSValue) -> c_int;
    fn JS_GetException(ctx: *mut JSContext) -> JSValue;
    fn breaky_free_value(ctx: *mut JSContext, v: JSValue);
    fn breaky_to_cstring(ctx: *mut JSContext, v: JSValue) -> *const c_char;
    fn JS_FreeCString(ctx: *mut JSContext, s: *const c_char);
    fn breaky_install_console(ctx: *mut JSContext);
}

const EVAL_GLOBAL: c_int = 0;

fn cstr(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

pub struct Runtime {
    rt: *mut JSRuntime,
    ctx: *mut JSContext,
    dom: *mut dom::DomState,
}

unsafe impl Send for Runtime {}

impl Runtime {
    pub fn new() -> Runtime {
        unsafe {
            let rt = JS_NewRuntime();
            let ctx = JS_NewContext(rt);
            breaky_install_console(ctx);
            Runtime {
                rt,
                ctx,
                dom: std::ptr::null_mut(),
            }
        }
    }

    pub fn bind_dom(&mut self, root: &mut Node) {
        dom::install(self, root);
    }

    pub fn dispatch_event(
        &mut self,
        typ: &str,
        path: &[usize],
        x: f32,
        y: f32,
        key: &str,
    ) -> Result<bool, String> {
        dom::dispatch_event(self, typ, path, x, y, key)
    }

    pub fn dispatch_move(&mut self, path: &[usize], x: f32, y: f32) -> Result<bool, String> {
        dom::dispatch_move(self, path, x, y)
    }

    pub fn set_rects(&mut self, rects: &HashMap<Vec<usize>, (f32, f32, f32, f32)>) {
        dom::set_rects(self, rects);
    }

    pub fn eval(&mut self, src: &str) -> Result<String, String> {
        let name = cstr("script");
        let src = cstr(src);
        unsafe {
            let v = JS_Eval(
                self.ctx,
                src.as_ptr(),
                src.as_bytes().len(),
                name.as_ptr(),
                EVAL_GLOBAL,
            );
            if breaky_is_exception(v) != 0 {
                let e = JS_GetException(self.ctx);
                let msg = to_string(self.ctx, e);
                breaky_free_value(self.ctx, e);
                Err(msg)
            } else {
                let out = to_string(self.ctx, v);
                breaky_free_value(self.ctx, v);
                Ok(out)
            }
        }
    }
}

unsafe fn to_string(ctx: *mut JSContext, v: JSValue) -> String {
    let s = breaky_to_cstring(ctx, v);
    if s.is_null() {
        return String::new();
    }
    let out = std::ffi::CStr::from_ptr(s).to_string_lossy().into_owned();
    JS_FreeCString(ctx, s);
    out
}

impl Default for Runtime {
    fn default() -> Runtime {
        Runtime::new()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            JS_FreeContext(self.ctx);
            JS_FreeRuntime(self.rt);
        }
        if !self.dom.is_null() {
            drop(unsafe { Box::from_raw(self.dom) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn eval_arithmetic() {
        let mut r = Runtime::new();
        assert_eq!(r.eval("1 + 2 * 3").unwrap(), "7");
    }


    #[test]
    fn eval_strings_and_state() {
        let mut r = Runtime::new();
        r.eval("var x = 40; x += 2;").unwrap();
        assert_eq!(r.eval("String(x) + \"!\"").unwrap(), "42!");
    }

    #[test]
    fn eval_function_and_closures() {
        let mut r = Runtime::new();
        assert_eq!(
            r.eval("function fib(n){ return n < 2 ? n : fib(n-1) + fib(n-2) }; fib(15)").unwrap(),
            "610"
        );
    }

    #[test]
    fn eval_error_message() {
        let mut r = Runtime::new();
        let e = r.eval("null.foo").unwrap_err();
        assert!(e.contains("TypeError"), "{}", e);
    }

    #[test]
    fn eval_objects_and_json() {
        let mut r = Runtime::new();
        assert_eq!(
            r.eval("JSON.stringify({a: 1, b: [2, 3]})").unwrap(),
            "{\"a\":1,\"b\":[2,3]}"
        );
    }
}
