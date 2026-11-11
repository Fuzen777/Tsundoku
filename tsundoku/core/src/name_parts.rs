//! Names made of text and placeholders' values, joined with every placeholder that has no value left out along with
//! the text that separates it, so no name says "None" or has two separators in a row: posts' file names
//! (`post_names`), and a rule's folder and file name (`rules`).

/// A piece of a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamePart {
    /// Text as it is, such as a separator.
    Text(String),
    /// A placeholder's value, or `None` when it has none.
    Value(Option<String>),
}

impl NamePart {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Whether it's a placeholder without a value.
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Value(None))
    }
}

/// The parts joined, each value that's `None` left out with the text before it, or with the text after it when no
/// value before it is kept: "{account} - {date} - {post}" without a date is "{account} - {post}", and without an
/// account "{date} - {post}". Text after a value goes only when something follows it, so a name's own ending stays.
pub fn join_leaving_out(parts: &[NamePart]) -> String {
    let mut kept = vec![true; parts.len()];
    for (index, part) in parts.iter().enumerate() {
        if !part.is_missing() {
            continue;
        }
        kept[index] = false;
        let earlier_value = parts[..index]
            .iter()
            .enumerate()
            .any(|(at, part)| kept[at] && matches!(part, NamePart::Value(_)));
        let separator = if earlier_value {
            index.checked_sub(1)
        } else {
            Some(index + 1).filter(|next| *next + 1 < parts.len())
        };
        if let Some(separator) = separator
            && matches!(parts.get(separator), Some(NamePart::Text(_)))
        {
            kept[separator] = false;
        }
    }
    parts
        .iter()
        .zip(kept)
        .filter(|(_, kept)| *kept)
        .filter_map(|(part, _)| match part {
            NamePart::Text(text) => Some(text.as_str()),
            NamePart::Value(value) => value.as_deref(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(text: &str) -> NamePart {
        NamePart::Value(Some(text.to_owned()))
    }

    const MISSING: NamePart = NamePart::Value(None);

    #[test]
    fn leaves_out_a_missing_value_with_the_text_before_it() {
        let parts = |date: NamePart| {
            vec![
                value("natgeo"),
                NamePart::text(" - "),
                date,
                NamePart::text(" - "),
                value("DfPQchkG1tg"),
            ]
        };
        assert_eq!(
            join_leaving_out(&parts(value("2026-11-04"))),
            "natgeo - 2026-11-04 - DfPQchkG1tg"
        );
        assert_eq!(join_leaving_out(&parts(MISSING)), "natgeo - DfPQchkG1tg");
        assert_eq!(
            join_leaving_out(&[value("RJ01"), NamePart::text(" by "), MISSING]),
            "RJ01",
            "the text before it, words included"
        );
    }

    #[test]
    fn leaves_out_a_first_missing_value_with_the_text_after_it() {
        assert_eq!(
            join_leaving_out(&[MISSING, NamePart::text("_"), value("RJ01")]),
            "RJ01"
        );
        assert_eq!(
            join_leaving_out(&[
                MISSING,
                NamePart::text(" - "),
                MISSING,
                NamePart::text(" - "),
                value("x")
            ]),
            "x"
        );
        assert_eq!(
            join_leaving_out(&[NamePart::text("By "), MISSING]),
            "By ",
            "text that ends the name stays"
        );
        assert_eq!(
            join_leaving_out(&[MISSING, NamePart::text(" "), MISSING]),
            ""
        );
    }
}
