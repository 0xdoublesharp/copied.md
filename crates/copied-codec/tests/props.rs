//! Property-based round trips mirroring the Lean theorems.

use copied_codec::cf_html::{encode_cf_html, parse_cf_html};
use copied_codec::pickle::{decode_chromium_custom_data, encode_chromium_custom_data};
use copied_codec::rtf::{escape_rtf, unescape_rtf};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// `CopiedFormal.Pickle.decode_encode_roundtrip` and `encode_aligned`.
    #[test]
    fn pickle_round_trip(entries in prop::collection::vec((any::<String>(), any::<String>()), 0..6)) {
        let borrowed: Vec<(&str, &str)> = entries.iter().map(|(m, v)| (m.as_str(), v.as_str())).collect();
        let bytes = encode_chromium_custom_data(&borrowed).unwrap();
        prop_assert_eq!(bytes.len() % 4, 0);
        let declared = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        prop_assert_eq!(declared as usize, bytes.len() - 4);
        prop_assert_eq!(decode_chromium_custom_data(&bytes).unwrap(), entries);
    }

    /// The strict decoder never panics on arbitrary bytes.
    #[test]
    fn pickle_decode_total(bytes in prop::collection::vec(any::<u8>(), 0..64)) {
        let _ = decode_chromium_custom_data(&bytes);
    }

    /// `CopiedFormal.CfHtml.offsets_ordered`, `fragment_slice` and `endHtml_eq_length`.
    #[test]
    fn cf_html_round_trip(fragment in any::<String>()) {
        let payload = encode_cf_html(&fragment);
        let offsets = parse_cf_html(&payload).unwrap();
        prop_assert!(offsets.start_html < offsets.start_fragment);
        prop_assert!(offsets.start_fragment <= offsets.end_fragment);
        prop_assert!(offsets.end_fragment < offsets.end_html);
        prop_assert_eq!(offsets.end_html, payload.len());
        prop_assert_eq!(&payload[offsets.start_fragment..offsets.end_fragment], fragment.as_str());
    }

    /// `CopiedFormal.Rtf.rtf_output_ascii` and `unescape_escape_roundtrip`.
    #[test]
    fn rtf_round_trip(value in any::<String>()) {
        let escaped = escape_rtf(&value);
        prop_assert!(escaped.is_ascii());
        prop_assert_eq!(unescape_rtf(&escaped).unwrap(), value);
    }

    /// Line breaks and braces in the fragment never confuse the header parser.
    #[test]
    fn cf_html_multiline_fragment(fragment in r"[a-z:{}\r\n<>!-]{0,32}") {
        let payload = encode_cf_html(&fragment);
        let offsets = parse_cf_html(&payload).unwrap();
        prop_assert_eq!(&payload[offsets.start_fragment..offsets.end_fragment], fragment.as_str());
    }

    /// The unescaper never panics on arbitrary text.
    #[test]
    fn rtf_unescape_total(value in r"[\\u0-9?{}a-z-]{0,24}") {
        let _ = unescape_rtf(&value);
    }
}
