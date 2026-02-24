/// Extract a meta tag value from NZB XML content
pub fn extract_meta_tag(content: &str, tag_type: &str) -> Option<String> {
    let pattern = format!(r#"<meta type="{}">"#, tag_type);
    if let Some(start) = content.find(&pattern) {
        let after_tag = &content[start + pattern.len()..];
        if let Some(end) = after_tag.find("</meta>") {
            return Some(after_tag[..end].to_string());
        }
    }
    None
}
