#include <stdio.h>
#include "quickjs.h"

static JSValue console_log(JSContext *ctx, JSValueConst this, int argc, JSValueConst *argv) {
    for (int i = 0; i < argc; i++) {
        if (i) {
            fputc(' ', stdout);
        }
        const char *s = JS_ToCString(ctx, argv[i]);
        if (s) {
            fputs(s, stdout);
            JS_FreeCString(ctx, s);
        }
    }
    fputc('\n', stdout);
    return JS_UNDEFINED;
}

int breaky_is_exception(JSValue v) {
    return JS_IsException(v);
}

const char *breaky_to_cstring(JSContext *ctx, JSValue v) {
    return JS_ToCString(ctx, v);
}

void breaky_free_value(JSContext *ctx, JSValue v) {
    JS_FreeValue(ctx, v);
}

void breaky_install_console(JSContext *ctx) {
    JSValue global = JS_GetGlobalObject(ctx);
    JSValue console = JS_NewObject(ctx);
    JS_SetPropertyStr(ctx, console, "log", JS_NewCFunction(ctx, console_log, "log", 1));
    JS_SetPropertyStr(ctx, console, "error", JS_NewCFunction(ctx, console_log, "error", 1));
    JS_SetPropertyStr(ctx, console, "warn", JS_NewCFunction(ctx, console_log, "warn", 1));
    JS_SetPropertyStr(ctx, global, "console", console);
    JS_FreeValue(ctx, global);
}
