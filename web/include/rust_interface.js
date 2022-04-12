var rust_funcs = {};

function expose_closure(name, closure) {
    rust_funcs[name] = closure;
}
