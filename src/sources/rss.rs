//! Minimal, lenient RSS reader shared by the feed-based sources.

use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::pirateclient::ApiError;

/// One `<item>` of an RSS feed. Element text is keyed by tag name (e.g.
/// `title`, `nyaa:seeders`) and attributes by `tag@attr` (e.g.
/// `enclosure@url`). When a tag repeats, the first non-empty value wins.
#[derive(Default, Debug)]
pub(crate) struct RssItem(HashMap<String, String>);

impl RssItem {
    /// The value for `key`, or `""` when absent.
    pub fn get(&self, key: &str) -> &str {
        self.0.get(key).map(String::as_str).unwrap_or("")
    }

    fn set(&mut self, key: String, value: String) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        self.0
            .entry(key)
            .and_modify(|v| {
                if v.is_empty() {
                    *v = value.to_string()
                }
            })
            .or_insert_with(|| value.to_string());
    }

    fn set_attrs(&mut self, e: &BytesStart) {
        let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
        for attr in e.attributes().flatten() {
            let key = String::from_utf8_lossy(attr.key.as_ref());
            let value = attr
                .unescape_value()
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| String::from_utf8_lossy(&attr.value).into_owned());
            self.set(format!("{tag}@{key}"), value);
        }
    }
}

/// Parse every `<item>` of an RSS document. Parsing stops at `</rss>`, since
/// some sites append markup (e.g. injected scripts) after the document, and
/// malformed entities are kept verbatim rather than failing the whole feed.
pub(crate) fn parse_items(xml: &str) -> Result<Vec<RssItem>, ApiError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().check_end_names = false;

    let mut items = Vec::new();
    let mut item: Option<RssItem> = None;
    let mut tag = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if tag == "item" {
                    item = Some(RssItem::default());
                } else if let Some(item) = item.as_mut() {
                    item.set_attrs(&e);
                }
            }
            Ok(Event::Empty(e)) => {
                if let Some(item) = item.as_mut() {
                    item.set_attrs(&e);
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(item) = item.as_mut() {
                    let text = e
                        .unescape()
                        .map(|t| t.into_owned())
                        .unwrap_or_else(|_| String::from_utf8_lossy(&e).into_owned());
                    item.set(tag.clone(), text);
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(item) = item.as_mut() {
                    item.set(tag.clone(), String::from_utf8_lossy(&e).into_owned());
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"item" => items.extend(item.take()),
                b"rss" => break,
                _ => tag.clear(),
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(ApiError::Parse(e.to_string())),
        }
    }
    Ok(items)
}

/// The integer following `label` in free text, skipping `:` and whitespace —
/// e.g. `number_after("Seeds: 35 , Leechers 0", "Leechers") == Some(0)`.
pub(crate) fn number_after(text: &str, label: &str) -> Option<u32> {
    let rest = &text[text.find(label)? + label.len()..];
    let digits: String = rest
        .trim_start_matches(|c: char| c == ':' || c.is_whitespace())
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_text_cdata_and_attributes() {
        let xml = r#"<rss><channel><title>feed</title>
            <item>
              <title>A &amp; B</title>
              <category>Movies</category>
              <category domain="x">Ignored</category>
              <description><![CDATA[Seeds: 3<br />]]></description>
              <enclosure url="https://h/torrent/ABC.torrent?title=a b" />
            </item>
            </channel></rss>"#;
        let items = parse_items(xml).unwrap();
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(it.get("title"), "A & B");
        assert_eq!(it.get("category"), "Movies");
        assert_eq!(it.get("category@domain"), "x");
        assert_eq!(it.get("description"), "Seeds: 3<br />");
        assert_eq!(
            it.get("enclosure@url"),
            "https://h/torrent/ABC.torrent?title=a b"
        );
        assert_eq!(it.get("missing"), "");
    }

    #[test]
    fn tolerates_trailing_markup_and_bad_entities() {
        let xml = "<rss><channel><item><title>Tom & Jerry</title></item></channel></rss>\
                   <script>if (a < b) {}</script>";
        let items = parse_items(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].get("title"), "Tom & Jerry");
    }

    #[test]
    fn extracts_numbers_after_labels() {
        assert_eq!(number_after("Seeds: 35 , Leechers 0", "Seeds"), Some(35));
        assert_eq!(number_after("Seeds: 35 , Leechers 0", "Leechers"), Some(0));
        assert_eq!(
            number_after("Seeds: 0<br />Leechers: 7<br />", "Leechers"),
            Some(7)
        );
        assert_eq!(number_after("Peers: n/a", "Peers"), None);
        assert_eq!(number_after("nothing", "Seeds"), None);
    }
}
