use semver::Version;

use crate::semver::compatible;

macro_rules! v {
    ($v:expr) => {
        Version::parse($v).expect("hard-coded value should be valid")
    };
}

#[test]
fn tests() {
    let v0_0_0 = v!("0.0.0");
    let v0_0_1 = v!("0.0.1");
    let v0_0_2 = v!("0.0.2");
    let v0_1_0 = v!("0.1.0");
    let v0_1_1 = v!("0.1.1");
    let v0_2_0 = v!("0.2.0");
    let v1_0_0 = v!("1.0.0");
    let v1_0_0_build = v!("1.0.0+build");
    let v1_0_0_alpha = v!("1.0.0-alpha");
    let v1_0_0_beta = v!("1.0.0-beta");
    let v1_0_1 = v!("1.0.1");
    let v1_1_0 = v!("1.1.0");
    let v1_1_1 = v!("1.1.1");
    let v2_0_0 = v!("2.0.0");

    assert!(compatible(&v0_0_0, &v0_0_0));
    assert!(compatible(&v0_1_0, &v0_1_0));
    assert!(compatible(&v0_1_0, &v0_1_1));
    assert!(compatible(&v1_0_0, &v1_0_0));
    assert!(compatible(&v1_0_0, &v1_0_0_build));
    assert!(compatible(&v1_0_0, &v1_0_1));
    assert!(compatible(&v1_0_0, &v1_1_0));
    assert!(compatible(&v1_0_0, &v1_1_1));
    assert!(compatible(&v1_0_0_alpha, &v1_0_0_alpha));
    assert!(compatible(&v1_0_0_build, &v1_0_0));
    assert!(compatible(&v1_0_0_build, &v1_0_0_build));

    assert!(!compatible(&v0_0_0, &v0_0_1));
    assert!(!compatible(&v0_0_1, &v0_0_2));
    assert!(!compatible(&v0_1_0, &v0_2_0));
    assert!(!compatible(&v0_1_1, &v0_1_0));
    assert!(!compatible(&v1_0_0, &v2_0_0));
    assert!(!compatible(&v1_0_0_alpha, &v1_0_0_beta));
    assert!(!compatible(&v1_0_0_beta, &v1_0_0_alpha));
    assert!(!compatible(&v1_0_1, &v1_0_0));
    assert!(!compatible(&v1_1_0, &v1_0_0));
    assert!(!compatible(&v1_1_1, &v1_0_0));
}
