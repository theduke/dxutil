use std::fmt;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldPath(String);

impl FieldPath {
    pub fn root() -> Self {
        Self(String::new())
    }

    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn child(&self, segment: impl AsRef<str>) -> Self {
        let segment = segment.as_ref();
        if self.0.is_empty() {
            Self(segment.to_string())
        } else {
            Self(format!("{}.{}", self.0, segment))
        }
    }

    pub fn index(&self, index: usize) -> Self {
        if self.0.is_empty() {
            Self(format!("[{index}]"))
        } else {
            Self(format!("{}[{index}]", self.0))
        }
    }

    pub fn starts_with(&self, prefix: &FieldPath) -> bool {
        if prefix.is_root() {
            return true;
        }
        self.0 == prefix.0
            || self
                .0
                .strip_prefix(prefix.as_str())
                .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
    }
}

impl fmt::Display for FieldPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for FieldPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for FieldPath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_format_children_and_indexes() {
        let path = FieldPath::root()
            .child("people")
            .index(2)
            .child("hobbies")
            .index(1)
            .child("title");
        assert_eq!(path.as_str(), "people[2].hobbies[1].title");
    }

    #[test]
    fn prefix_matching_respects_boundaries() {
        let prefix = FieldPath::new("person.hobbies");
        assert!(FieldPath::new("person.hobbies[0].name").starts_with(&prefix));
        assert!(FieldPath::new("person.hobbies.title").starts_with(&prefix));
        assert!(!FieldPath::new("person.hobbies_extra").starts_with(&prefix));
    }
}
