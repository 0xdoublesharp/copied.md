use copied_core::ClipboardPayload;

use crate::WriteError;

#[derive(Debug)]
pub(crate) struct Bundle<'a> {
    pub plain: &'a str,
    pub html: Option<&'a str>,
    pub rtf: Option<&'a str>,
    pub signal_pickle: Option<Vec<u8>>,
}

impl Bundle<'_> {
    pub fn types(&self) -> Vec<String> {
        [
            Some("text/plain"),
            self.html.map(|_| "text/html"),
            self.rtf.map(|_| "text/rtf"),
            self.signal_pickle.as_ref().map(|_| "text/signal"),
        ]
        .into_iter()
        .flatten()
        .map(str::to_owned)
        .collect()
    }
}

pub(crate) fn prepare_bundle(payload: &ClipboardPayload) -> Result<Bundle<'_>, WriteError> {
    if payload.plain.trim().is_empty() {
        return Err(WriteError::Empty);
    }
    Ok(Bundle {
        plain: &payload.plain,
        html: payload.html.as_deref().filter(|text| !text.is_empty()),
        rtf: payload.rtf.as_deref().filter(|text| !text.is_empty()),
        signal_pickle: payload
            .signal
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(copied_codec::pickle::encode_signal_pickle)
            .transpose()
            .map_err(|error| WriteError::Rejected(error.to_string()))?,
    })
}

// Also compiled by the macOS integration check, independently of the Windows SDK.
#[cfg(any(test, target_os = "windows"))]
pub(crate) fn windows_html(fragment: &str) -> Vec<u8> {
    let mut html = copied_codec::cf_html::encode_cf_html(fragment).into_bytes();
    html.push(0);
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_flavors_use_codec_outputs() {
        let payload = ClipboardPayload {
            plain: "example".into(),
            html: Some("<p>Café</p>".into()),
            rtf: Some(r"{\rtf1 Example}".into()),
            signal: Some("<strong>🛰️</strong>".into()),
        };
        let bundle = prepare_bundle(&payload).unwrap();
        assert_eq!(
            bundle.types(),
            ["text/plain", "text/html", "text/rtf", "text/signal"]
        );
        assert_eq!(
            bundle.signal_pickle.unwrap(),
            copied_codec::pickle::encode_signal_pickle(payload.signal.as_ref().unwrap()).unwrap()
        );
        let fragment = bundle.html.unwrap();
        let html = windows_html(fragment);
        let envelope = std::str::from_utf8(html.strip_suffix(&[0]).unwrap()).unwrap();
        assert_eq!(envelope, copied_codec::cf_html::encode_cf_html(fragment));
        if envelope != fragment {
            let offsets = copied_codec::cf_html::parse_cf_html(envelope).unwrap();
            assert_eq!(
                &envelope[offsets.start_fragment..offsets.end_fragment],
                fragment
            );
        }
    }
}
