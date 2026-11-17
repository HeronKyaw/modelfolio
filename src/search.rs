//! One-box search with operators:
//! plain words match model id / owner / gateway name,
//! plus `tag:x`, `gateway:x`, `is:new`, `is:favorite`, `is:removed`, `is:hidden`.

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Query {
    pub words: Vec<String>,
    pub tags: Vec<String>,
    pub gateways: Vec<String>,
    pub is_new: bool,
    pub is_favorite: bool,
    pub is_removed: bool,
    pub is_hidden: bool,
}

impl Query {
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
            && self.tags.is_empty()
            && self.gateways.is_empty()
            && !self.is_new
            && !self.is_favorite
            && !self.is_removed
            && !self.is_hidden
    }
}

pub fn parse(q: &str) -> Query {
    let mut out = Query::default();
    for tok in q.split_whitespace() {
        let t = tok.to_ascii_lowercase();
        if let Some(rest) = t.strip_prefix("tag:") {
            if !rest.is_empty() {
                out.tags.push(rest.to_string());
            }
        } else if let Some(rest) = t.strip_prefix("gateway:") {
            if !rest.is_empty() {
                out.gateways.push(rest.to_string());
            }
        } else if let Some(rest) = t.strip_prefix("is:") {
            match rest {
                "new" => out.is_new = true,
                "favorite" | "starred" => out.is_favorite = true,
                "removed" => out.is_removed = true,
                "hidden" | "ignore" => out.is_hidden = true,
                _ => out.words.push(t),
            }
        } else {
            out.words.push(t);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_words() {
        let q = parse("llama 70b");
        assert_eq!(q.words, vec!["llama", "70b"]);
        assert!(q.is_empty() == false);
    }

    #[test]
    fn operators() {
        let q = parse("tag:coding gateway:groq is:new llama");
        assert_eq!(q.tags, vec!["coding"]);
        assert_eq!(q.gateways, vec!["groq"]);
        assert!(q.is_new);
        assert_eq!(q.words, vec!["llama"]);
    }

    #[test]
    fn all_flags() {
        let q = parse("is:favorite is:removed is:hidden");
        assert!(q.is_favorite && q.is_removed && q.is_hidden);
    }

    #[test]
    fn unknown_is_falls_back_to_word() {
        let q = parse("is:bogus");
        assert_eq!(q.words, vec!["is:bogus"]);
    }
}
