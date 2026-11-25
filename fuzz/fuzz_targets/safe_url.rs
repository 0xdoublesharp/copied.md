//! Fuzzes `copied_core::safe_url` with arbitrary destinations and base URLs.
//!
//! Every accepted URL must be a fragment or an `http:`, `https:` or `mailto:` URL, both by
//! its spelling and when parsed, and the function must be deterministic.
#![no_main]

use arbitrary::Arbitrary;
use copied_core::safe_url;
use libfuzzer_sys::fuzz_target;

const BASES: &[&str] = &[
    "https://example.org/docs/page",
    "http://example.org/",
    "mailto:hi@example.org",
    "file:///etc/passwd",
    "javascript:alert(1)",
    "data:text/html,x",
    "not a url",
];

const ALLOWED_PREFIXES: [&str; 4] = ["http:", "https:", "mailto:", "#"];

#[derive(Arbitrary, Debug)]
enum Base {
    None,
    Known(u8),
    Arbitrary(String),
}

#[derive(Arbitrary, Debug)]
struct Input {
    url: String,
    base: Base,
}

fuzz_target!(|input: Input| {
    let base = match &input.base {
        Base::None => None,
        Base::Known(index) => Some(BASES[usize::from(*index) % BASES.len()].to_owned()),
        Base::Arbitrary(base) => Some(base.clone()),
    };
    let result = safe_url(&input.url, base.as_deref());
    assert_eq!(
        result,
        safe_url(&input.url, base.as_deref()),
        "non-deterministic"
    );
    let Some(accepted) = result else {
        return;
    };
    let lower = accepted.to_ascii_lowercase();
    assert!(
        ALLOWED_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix)),
        "accepted {accepted:?} from {:?} with base {base:?}",
        input.url
    );
    if accepted.starts_with('#') {
        assert_eq!(accepted, input.url, "fragments are kept verbatim");
        return;
    }
    // Resolve the way a browser would: relative to the base when there is one. The original
    // spelling is kept, so `http:` alone is accepted against an `http:` base.
    let resolved = match base.as_deref() {
        Some(base) => url::Url::parse(base).and_then(|base| base.join(&accepted)),
        None => url::Url::parse(&accepted),
    };
    let parsed = resolved.unwrap_or_else(|e| panic!("accepted unresolvable {accepted:?} ({e})"));
    assert!(
        ["http", "https", "mailto"].contains(&parsed.scheme()),
        "accepted {accepted:?} parses as {}",
        parsed.scheme()
    );
});
