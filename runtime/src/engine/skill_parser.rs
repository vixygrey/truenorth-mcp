//! SKILL.md structural parser (ports `parseSkillMarkdown` in `skill-parser.ts`).
//!
//! `parse_skill` reads a skill's raw markdown and returns its frontmatter, headings, code
//! blocks, sections, links, and joined prose. The legacy `read_skill` tool returns this
//! shape. Parsing is a pull parse over CommonMark events, so it stays faithful to the
//! legacy remark-based structure.
//!
//! Requirements: 7.2, 7.3. Design: Part II §1.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde::Serialize;
use serde_yaml::Value;

use crate::engine::skill::RawSkill;

/// A heading in a skill (depth and text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Heading {
    /// The heading level, 1 for `#`, 2 for `##`, and so on.
    pub depth: u8,
    /// The heading text.
    pub text: String,
}

/// A fenced code block (language tag and body).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeBlock {
    /// The language tag, or `None` for a bare fence.
    pub lang: Option<String>,
    /// The code body.
    pub value: String,
}

/// A section: the prose that follows a heading, keyed by that heading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    /// The heading text, or `None` for prose before the first heading.
    pub heading: Option<String>,
    /// The joined prose under the heading.
    pub prose: String,
}

/// A markdown link (text and url).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    /// The link text.
    pub text: String,
    /// The link URL.
    pub url: String,
}

/// A parsed skill (ports `ParsedSkill`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParsedSkill {
    /// The skill directory name.
    pub name: String,
    /// The repo-relative path to the `SKILL.md`.
    pub path: PathBuf,
    /// The YAML frontmatter as a key-value map.
    pub frontmatter: BTreeMap<String, Value>,
    /// Every heading, in document order.
    pub headings: Vec<Heading>,
    /// Every fenced code block, in document order.
    pub code_blocks: Vec<CodeBlock>,
    /// Every section, split on headings.
    pub sections: Vec<Section>,
    /// Every link, in document order.
    pub links: Vec<Link>,
    /// The joined prose of every paragraph.
    pub raw_prose: String,
    /// Whether the source was truncated at the byte cap.
    pub truncated: bool,
}

/// Parse a raw skill's markdown into its structure (ports `parseSkillMarkdown`).
pub fn parse_skill(raw: &RawSkill) -> ParsedSkill {
    let (frontmatter_block, body) = extract_frontmatter(&raw.markdown);
    let frontmatter = parse_frontmatter(frontmatter_block);
    let mut parts = MarkdownParts::default();
    parts.walk(body);
    parts.flush_section();

    let raw_prose = parts.prose_parts.join("\n");
    ParsedSkill {
        name: raw.name.clone(),
        path: raw.path.clone(),
        frontmatter,
        headings: parts.headings,
        code_blocks: parts.code_blocks,
        sections: parts.sections,
        links: parts.links,
        raw_prose,
        truncated: raw.truncated,
    }
}

/// Split the leading YAML frontmatter block from the body (ports
/// `extractFrontmatterBlock`).
///
/// The block is the text between the first `---` line and the next `---` line. When a
/// delimiter is missing, the block is empty and the whole input is the body.
fn extract_frontmatter(raw: &str) -> (&str, &str) {
    let Some(start) = raw.find("---") else {
        return ("", raw);
    };
    let Some(line_end) = raw[start..].find('\n').map(|i| start + i) else {
        return ("", raw);
    };
    let Some(close_rel) = raw[line_end..].find("\n---") else {
        return ("", raw);
    };
    let close = line_end + close_rel;
    let block = &raw[line_end + 1..close];
    // The body starts after the closing `\n---` and its line end.
    let after_close = &raw[close + 4..];
    let body = after_close.strip_prefix('\n').unwrap_or(after_close);
    (block, body)
}

/// Parse the frontmatter YAML into a map (ports `parseFrontmatterYaml`).
///
/// An empty block yields an empty map. A parse failure yields an empty map rather than an
/// error, matching the legacy tolerance: a malformed frontmatter never fails a read.
fn parse_frontmatter(block: &str) -> BTreeMap<String, Value> {
    if block.trim().is_empty() {
        return BTreeMap::new();
    }
    serde_yaml::from_str::<BTreeMap<String, Value>>(block).unwrap_or_default()
}

/// The accumulator that walks CommonMark events into the parsed parts.
#[derive(Default)]
struct MarkdownParts {
    headings: Vec<Heading>,
    code_blocks: Vec<CodeBlock>,
    links: Vec<Link>,
    prose_parts: Vec<String>,
    sections: Vec<Section>,
    // Section-in-progress state.
    current_heading: Option<String>,
    current_section_prose: Vec<String>,
    // Event-in-progress state.
    in_heading: Option<u8>,
    heading_text: String,
    in_code: Option<Option<String>>,
    code_text: String,
    in_paragraph: bool,
    paragraph_text: String,
    in_link: Option<String>,
    link_text: String,
}

impl MarkdownParts {
    /// Walk the body's CommonMark events, filling the parts.
    fn walk(&mut self, body: &str) {
        let parser = Parser::new_ext(body, Options::empty());
        for event in parser {
            self.handle(event);
        }
    }

    /// Handle one CommonMark event.
    fn handle(&mut self, event: Event) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                self.flush_section();
                self.in_heading = Some(level as u8);
                self.heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(depth) = self.in_heading.take() {
                    let text = self.heading_text.trim().to_string();
                    self.headings.push(Heading {
                        depth,
                        text: text.clone(),
                    });
                    self.current_heading = Some(text);
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(tag) if !tag.is_empty() => Some(tag.to_string()),
                    _ => None,
                };
                self.in_code = Some(lang);
                self.code_text.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(lang) = self.in_code.take() {
                    self.code_blocks.push(CodeBlock {
                        lang,
                        value: self.code_text.clone(),
                    });
                }
            }
            Event::Start(Tag::Paragraph) => {
                self.in_paragraph = true;
                self.paragraph_text.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                self.in_paragraph = false;
                let text = self.paragraph_text.trim().to_string();
                if !text.is_empty() {
                    self.prose_parts.push(text.clone());
                    self.current_section_prose.push(text);
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                // Capture the URL from the start tag; the text accumulates until the end.
                self.in_link = Some(dest_url.to_string());
                self.link_text.clear();
            }
            Event::End(TagEnd::Link) => {
                if let Some(url) = self.in_link.take() {
                    self.links.push(Link {
                        text: self.link_text.trim().to_string(),
                        url,
                    });
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if self.in_heading.is_some() {
                    self.heading_text.push_str(&text);
                }
                if self.in_code.is_some() {
                    self.code_text.push_str(&text);
                }
                if self.in_paragraph {
                    self.paragraph_text.push_str(&text);
                }
                if self.in_link.is_some() {
                    self.link_text.push_str(&text);
                }
            }
            _ => {}
        }
    }

    /// Flush the current section into the section list.
    fn flush_section(&mut self) {
        if !self.current_section_prose.is_empty() || self.current_heading.is_some() {
            self.sections.push(Section {
                heading: self.current_heading.clone(),
                prose: self.current_section_prose.join("\n").trim().to_string(),
            });
            self.current_section_prose.clear();
        }
    }
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `skill_parser`.
#[cfg(test)]
#[path = "skill_parser_tests.rs"]
mod tests;
