use rf_dsp::fixture::{deembed, parse, split};
#[test]
fn asymmetric_launches_match_scikit_rf_nzc_golden() {
    let thru = parse(include_str!("data/nzc-input.s2p")).unwrap();
    let expected_l = parse(include_str!("data/nzc-left.s2p")).unwrap();
    let expected_r = parse(include_str!("data/nzc-right.s2p")).unwrap();
    let e = split(&thru, Some(50.)).unwrap();
    let error = e
        .left
        .s
        .iter()
        .zip(&expected_l.s)
        .chain(e.right.s.iter().zip(&expected_r.s))
        .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| (*a - *b).norm2().sqrt()))
        .fold(0., f64::max);
    assert!(error < 2e-5, "Complex error vs scikit-rf 1.8.0 = {error}");
    assert!(e.residual_db < 0.1 && e.residual_deg < 1.);
    let wrong = deembed(&thru, &e.left, &e.right, false, false).unwrap();
    assert!(
        wrong
            .s
            .iter()
            .zip(&thru.s)
            .any(|(s, _)| s[0].norm2() > 1e-6)
    );
}
