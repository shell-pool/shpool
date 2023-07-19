use std::env;

fn main() {
    let ac = autocfg::new();

    ac.emit_expression_cfg(
        "unsafe { 1f64.to_int_unchecked::<i32>() }",
        "has_to_int_unchecked",
    );

    ac.emit_expression_cfg("1u32.reverse_bits()", "has_reverse_bits");
    ac.emit_expression_cfg("1u32.trailing_ones()", "has_leading_trailing_ones");
    ac.emit_expression_cfg("1u32.div_euclid(1u32)", "has_div_euclid");

    if env::var_os("CARGO_FEATURE_STD").is_some() {
        ac.emit_expression_cfg("1f64.copysign(-1f64)", "has_copysign");
    }
    ac.emit_expression_cfg("1f64.is_subnormal()", "has_is_subnormal");

    ac.emit_expression_cfg("1u32.to_ne_bytes()", "has_int_to_from_bytes");
    ac.emit_expression_cfg("3.14f64.to_ne_bytes()", "has_float_to_from_bytes");

    autocfg::rerun_path("build.rs");
}
