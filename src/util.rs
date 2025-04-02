pub fn normalise_name(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "истина" => "istina".to_string(),
        "зима" => "zima".to_string(),
        "гум" => "gummy".to_string(),
        "лето" => "leto".to_string(),
        "роса" => "rosa".to_string(),
        name => name
            .replace(' ', "")
            .replace('-', "")
            .replace('\'', "")
            .replace('ł', "l")
            .replace('š', "s")
            .replace('"', ""),
    }
}
