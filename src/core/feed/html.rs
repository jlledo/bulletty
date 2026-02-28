use std::borrow::Cow;
use std::fmt;

use tl::{Bytes, Node, VDom};
use url::Url;

pub fn is_html(content: &str) -> bool {
    let trimmed = content.trim_start();
    trimmed.starts_with("<!DOCTYPE html")
        || trimmed.starts_with("<html")
        || trimmed.starts_with("<HTML")
}

pub struct Parser<'input> {
    dom: VDom<'input>,
    inner_iterator: Box<dyn Iterator<Item = String>>,
}

impl<'input> Parser<'input> {
    pub fn new(input: &str, url: Url) -> Result<Parser<'input>, ParseError> {
        let dom = tl::parse(input, tl::ParserOptions::default())?;
        let iterator = Self::feed_urls(&dom, url);

        Ok(Parser {
            dom,
            inner_iterator: Box::new(iterator),
        })
    }

    fn feed_urls(dom: &VDom<'_>, url: Url) -> impl Iterator<Item = String> {
        dom.query_selector("link[rel='alternate']")
            .into_iter()
            .flatten()
            .filter_map(move |node_handle| {
                node_handle
                    .get(dom.parser())
                    .and_then(Node::as_tag)
                    .filter(|tag| Self::get_attribute(tag, "type").is_some_and(Self::is_feed))
                    .and_then(|tag| Self::get_attribute(tag, "href"))
                    .and_then(|href| url.join(&href).map(String::from).ok())
            })
    }

    fn get_attribute<'a>(tag: &'a tl::HTMLTag<'a>, attribute: &'a str) -> Option<Cow<'a, str>> {
        tag.attributes()
            .get(attribute)
            .flatten()
            .map(Bytes::as_utf8_str)
    }

    fn is_feed(link_type: Cow<'_, str>) -> bool {
        let link_type = link_type.to_lowercase();
        link_type.contains("atom") || link_type.contains("rss")
    }
}

impl Iterator for Parser<'_> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner_iterator.next()
    }
}

/// An error that occurred during parsing
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParseError {
    /// The input string length was too large to fit in a `u32`
    TooLarge,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::TooLarge => write!(f, "The input string length was too large to fit in a `u32`"),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<tl::ParseError> for ParseError {
    fn from(value: tl::ParseError) -> Self {
        match value {
            tl::ParseError::InvalidLength => ParseError::TooLarge,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_absolute_url() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
<title>My Blog</title>
<link rel="alternate" type="application/rss+xml" href="https://example.com/feed.rss" />
</head>
<body>
<h1>Welcome</h1>
</body>
</html>"#;

        let mut parser = Parser::new(html, Url::parse("https://example.com/").unwrap()).unwrap();
        assert_eq!(parser.next(), Some("https://example.com/feed.rss".into()));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn extract_relative_url() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
<title>My Blog</title>
<link rel="alternate" type="application/atom+xml" href="/feed.atom" />
</head>
<body>
<h1>Welcome</h1>
</body>
</html>"#;

        let mut parser =
            Parser::new(html, Url::parse("https://example.com/blog/").unwrap()).unwrap();
        assert_eq!(parser.next(), Some("https://example.com/feed.atom".into()));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn extract_mixed_feed_urls() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
<title>Multi-feed Site</title>
<link rel="alternate" type="application/rss+xml" href="https://example.com/rss" />
<link rel="alternate" type="application/atom+xml" href="https://example.com/atom" />
</head>
<body>
<h1>Welcome</h1>
</body>
</html>"#;

        let mut parser = Parser::new(html, Url::parse("https://example.com/").unwrap()).unwrap();
        assert_eq!(parser.next(), Some("https://example.com/rss".into()));
        assert_eq!(parser.next(), Some("https://example.com/atom".into()));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn extract_no_urls() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
<title>No Feed Site</title>
</head>
<body>
<h1>Welcome</h1>
</body>
</html>"#;

        let mut parser = Parser::new(html, Url::parse("https://example.com/").unwrap()).unwrap();
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn html_doctype_is_html() {
        assert!(is_html("<!DOCTYPE html><html></html>"));
    }

    #[test]
    fn html_doctype_with_leading_whitespace_is_html() {
        assert!(is_html("  \n  <!DOCTYPE html><html></html>"));
    }

    #[test]
    fn html_tag_is_html() {
        assert!(is_html("<html><head></head></html>"));
    }

    #[test]
    fn rss_is_not_html() {
        assert!(!is_html("<?xml version=\"1.0\"?><rss></rss>"));
    }

    #[test]
    fn atom_is_not_html() {
        assert!(!is_html("<?xml version=\"1.0\"?><feed></feed>"));
    }
}
