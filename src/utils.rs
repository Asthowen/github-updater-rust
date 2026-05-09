pub fn next_link_header(header: &str) -> Option<String> {
    for link_value in header.split(',') {
        let mut segments = link_value.split(';');
        let url = segments
            .next()?
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>');

        for param in segments {
            if let Some((name, param_value)) = param.trim().split_once('=')
                && name == "rel"
                && param_value.trim_matches('"') == "next"
            {
                return Some(url.to_owned());
            }
        }
    }

    None
}

#[cfg(test)]
mod test {
    use super::next_link_header;

    #[test]
    fn extracts_next_link_from_header() {
        const HEADER_VALUE: &str = r#"<https://api.github.com/repositories/105379569/tags?page=2>; rel="next", <https://api.github.com/repositories/105379569/tags?page=17>; rel="last""#;
        assert_eq!(
            next_link_header(HEADER_VALUE).as_deref(),
            Some("https://api.github.com/repositories/105379569/tags?page=2")
        );
    }
}
