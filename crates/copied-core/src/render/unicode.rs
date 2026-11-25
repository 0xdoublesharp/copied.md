//! Mathematical Sans-Serif styled text (`unicode.ts`).

fn map(text: &str, upper: u32, lower: u32, digits: bool) -> String {
    text.chars()
        .map(|ch| {
            let offset = match ch {
                'A'..='Z' => Some(upper + (u32::from(ch) - u32::from('A'))),
                'a'..='z' => Some(lower + (u32::from(ch) - u32::from('a'))),
                '0'..='9' if digits => Some(0x1D7EC + (u32::from(ch) - u32::from('0'))),
                _ => None,
            };
            offset.and_then(char::from_u32).unwrap_or(ch)
        })
        .collect()
}

/// Mathematical Sans-Serif Bold letters and digits.
pub fn to_bold(text: &str) -> String {
    map(text, 0x1D5D4, 0x1D5EE, true)
}

/// Mathematical Sans-Serif Italic letters.
pub fn to_italic(text: &str) -> String {
    map(text, 0x1D608, 0x1D622, false)
}

/// Mathematical Sans-Serif Bold Italic letters with bold digits.
pub fn to_bold_italic(text: &str) -> String {
    map(text, 0x1D63C, 0x1D656, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_letters_and_digits() {
        assert_eq!(to_bold("Az09!"), "𝗔𝘇𝟬𝟵!");
        assert_eq!(to_italic("Az09"), "𝘈𝘻09");
        assert_eq!(to_bold_italic("Az9é"), "𝘼𝙯𝟵é");
    }
}
