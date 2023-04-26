pub fn lit_to_str(lit: &syn::Lit) -> String {
    match lit {
        syn::Lit::Bool(s) => s.value.to_string(),
        syn::Lit::Str(s) => string_to_identifier(&s.value()),
        syn::Lit::Int(s) => number_to_identifier(s.base10_digits()),
        syn::Lit::Float(s) => number_to_identifier(s.base10_digits()),
        _ => unimplemented!("String conversion for literal. Only bool, str, positive int, and float values are supported."),
    }
}

fn number_to_identifier(num: &str) -> String {
    num.chars()
        .map(|x| match x {
            '.' => 'd',
            '0'..='9' => x,
            '-' => 'n',
            _ => panic!("This is not a valid number. Contains unknown sign {}", x),
        })
        .collect()
}

fn string_to_identifier(num: &str) -> String {
    num.chars()
        .map(|x| match x {
            '0'..='9' => x.to_string(),
            'a'..='z' => x.to_string(),
            'A'..='Z' => x.to_string(),
            '!' => "_exclamation".to_string(),
            '"' => "_double_quote".to_string(),
            '#' => "_hash".to_string(),
            '$' => "_dollar".to_string(),
            '%' => "_percent".to_string(),
            '&' => "_ampercand".to_string(),
            '\'' => "_quote".to_string(),
            '(' => "_left_paranthesis".to_string(),
            ')' => "_right_paranthesis".to_string(),
            '*' => "_asterisk".to_string(),
            '+' => "_plus".to_string(),
            ',' => "_comma".to_string(),
            '-' => "_minus".to_string(),
            '.' => "_full_stop".to_string(),
            '/' => "_slash".to_string(),
            ':' => "_colon".to_string(),
            ';' => "_semicolon".to_string(),
            '<' => "_less_than".to_string(),
            '=' => "_equal".to_string(),
            '>' => "_greater_than".to_string(),
            '?' => "_questionmark".to_string(),
            '@' => "_at".to_string(),
            '[' => "_left_bracket".to_string(),
            '\\' => "_back_slash".to_string(),
            ']' => "_right_bracket".to_string(),
            '^' => "_caret".to_string(),
            '`' => "_backtick".to_string(),
            '{' => "_left_brace".to_string(),
            '|' => "_vertical_bar".to_string(),
            '}' => "_right_brace".to_string(),
            '~' => "_tilde".to_string(),
            _ => '_'.to_string(),
        })
        .collect()
}
