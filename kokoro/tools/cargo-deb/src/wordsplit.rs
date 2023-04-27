pub trait WordSplit {
    fn split_by_chars(&self, length: usize) -> Vec<String>;
}

impl WordSplit for str {
    // ref: https://www.debian.org/doc/debian-policy/ch-controlfields.html#description
    //
    // * Extended description line must have at least one non-whitespace character.
    //   If you violate this rule, `dpkg -i` will fail.
    // * Extended description line must not have any tab character.
    //   If you violate this rule, the effect is not predictable.
    //
    // NOTE: as for extended description, this splitting might not be necessary in the first place?
    // (debian policy seems to say nothing about line length of extended description)
    fn split_by_chars(&self, length: usize) -> Vec<String> {
        let output_capacity = self.len() + self.len() % length + 1;
        let mut lines: Vec<String> = Vec::with_capacity(output_capacity);
        let mut current_line = String::with_capacity(length);
        for line in self.lines() {
            // consider whitespace line as empty
            if line.chars().all(char::is_whitespace) {
                lines.push(String::from("."));
                continue;
            }

            let line = line.replace('\t', "  ");

            current_line.clear();
            let mut has_words = false;
            macro_rules! append_word {
                ($word:expr) => {{
                    if has_words {
                        current_line += " ";
                    }
                    has_words = true;
                    current_line.push_str($word);
                }};
            }
            for word in line.split(' ') {
                // we need at least one non-whitespace character
                if current_line.chars().all(char::is_whitespace) {
                    append_word!(word);
                    continue;
                }

                // now current_line has non-whitespace character
                if current_line.len() + word.len() >= length {
                    // skip a space
                    let to_push = std::mem::replace(&mut current_line, word.to_owned());
                    // if character length met or exceeded
                    lines.push(to_push);
                } else {
                    append_word!(word);
                }
            }

            // current_line may be trailing whitespaces
            if current_line.chars().all(char::is_whitespace) {
                lines.push(String::from("."));
            } else {
                lines.push(current_line.clone());
            }
        }
        lines
    }
}

#[test]
fn test_split_by_chars() {
    #[allow(non_snake_case)]
    fn S(s: &'static str) -> String { s.to_owned() }

    assert_eq!("This is a test string for split_by_chars.".split_by_chars(10), vec![
        S("This is a"),
        S("test"),
        S("string for"),
        S("split_by_chars.")
    ]);

    assert_eq!("This is a line\n\nthis is also a line.".split_by_chars(79), vec![
        S("This is a line"),
        S("."),
        S("this is also a line."),
    ]);

    assert_eq!("                                              verylongwordverylongwordverylongwordverylongword\n\nbo".split_by_chars(10), vec![
        S("                                              verylongwordverylongwordverylongwordverylongword"),
        S("."),
        S("bo"),
    ]);

    assert_eq!("This is a line\n  \nthis is also a line.\n".split_by_chars(79), vec![
        S("This is a line"),
        S("."),
        S("this is also a line."),
    ]);

    assert_eq!("    This  is an 4-indented line\n".split_by_chars(79), vec![
        S("    This  is an 4-indented line"),
    ]);

    assert_eq!("    This  is an 4-indented line\n".split_by_chars(3), vec![
        S("    This"),
        S(" is"),
        S("an"),
        S("4-indented"),
        S("line"),
    ]);

    assert_eq!("    indent,    then space".split_by_chars(4), vec![
        S("    indent,"),
        S("   then"),
        S("space"),
    ]);

    assert_eq!("  trailing space    ".split_by_chars(12), vec![
        S("  trailing"),
        S("space    "),
    ]);

    assert_eq!("  trailing space    ".split_by_chars(16), vec![
        S("  trailing space"),
        S("."),
    ]);

    assert_eq!("sh\nverylongwordverylongwordverylongwordverylongword\nend".split_by_chars(5), vec![
        S("sh"),
        S("verylongwordverylongwordverylongwordverylongword"),
        S("end"),
    ]);

    // from alacritty
    assert_eq!("       src=\"https://cloud.githubusercontent.com/assets/4285147/21585004/2ebd0288-d06c-11e6-95d3-4a2889dbbd6f.png\">".split_by_chars(79), vec![
        S("       src=\"https://cloud.githubusercontent.com/assets/4285147/21585004/2ebd0288-d06c-11e6-95d3-4a2889dbbd6f.png\">"),
    ]);

    assert_eq!("\t\ttabs are\treplaced with spaces\t".split_by_chars(10), vec![
        S("    tabs"),
        S("are "),
        S("replaced"),
        S("with"),
        S("spaces  "),
    ]);
}
