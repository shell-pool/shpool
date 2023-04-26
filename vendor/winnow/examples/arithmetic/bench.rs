mod parser;

use parser::expr;

#[allow(clippy::eq_op, clippy::erasing_op)]
fn arithmetic(c: &mut criterion::Criterion) {
    let data = "  2*2 / ( 5 - 1) + 3 / 4 * (2 - 7 + 567 *12 /2) + 3*(1+2*( 45 /2));";

    assert_eq!(
        expr(data),
        Ok((";", 2 * 2 / (5 - 1) + 3 * (1 + 2 * (45 / 2)),))
    );
    c.bench_function("arithmetic", |b| {
        b.iter(|| expr(data).unwrap());
    });
}

criterion::criterion_group!(benches, arithmetic);
criterion::criterion_main!(benches);
