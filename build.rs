use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let cubiomes_dir = format!("{}/vendor/cubiomes", manifest_dir);
    let include_dir = format!("{}/include", cubiomes_dir);

    println!("cargo:rustc-link-search=native={}", cubiomes_dir);
    println!("cargo:rustc-link-lib=dylib=cubiomes");
    println!("cargo:rustc-link-lib=dylib=m");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", cubiomes_dir);
    println!("cargo:rerun-if-changed={}/libcubiomes.so", cubiomes_dir);

    let bindings = bindgen::Builder::default()
        // 用一个 wrapper 头一次性包含所有 cubiomes 头文件
        .header_contents("wrapper.h", r#"
            #include "finders.h"
            #include "generator.h"
            #include "biomes.h"
            #include "layers.h"
            #include "noise.h"
            #include "biomenoise.h"
            #include "terrainnoise.h"
            #include "quadbase.h"
            #include "xrms.h"
            #include "carver.h"
            #include "rng.h"
            #include "util.h"
        "#)
        .clang_arg(format!("-I{include_dir}"))
        // 全量生成，但做以下几件事让代码可用：
        .generate_inline_functions(true)   // cubiomes 有些 inline 工具函数
        .derive_debug(true)
        .derive_default(true)
        .layout_tests(false)               // 关掉，省编译时间
        .opaque_type("jmp_buf")            // 遇到 setjmp 相关类型避免报错
        // ---- 只屏蔽 glibc math.h 的重复符号 ----
        .blocklist_item("^FP_.*$")         // FP_NAN/FP_INFINITE/FP_ZERO...
        .blocklist_item("^HUGE_VAL.*$")
        .blocklist_item("^INFINITY$")
        .blocklist_item("^NAN$")
        .blocklist_item("^math_errhandling$")
        .blocklist_item("^MATH_ERRNO$")
        .blocklist_item("^MATH_ERREXCEPT$")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("bindgen 生成失败");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("写入 bindings.rs 失败");
}