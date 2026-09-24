pub struct Parser<'a> {
    arguments: &'a [String],
    index: usize,
    inline_value: Option<&'a str>,
    option: bool,
}

impl<'a> Parser<'a> {
    pub fn new(arguments: &'a [String]) -> Self {
        Self {
            arguments,
            index: 0,
            inline_value: None,
            option: false,
        }
    }

    pub fn next(&mut self) -> Option<&'a str> {
        self.inline_value = None;
        let raw = self.arguments.get(self.index)?.as_str();
        self.index += 1;
        self.option = raw.starts_with('-') && raw != "-";
        let option = raw
            .strip_prefix("--")
            .or_else(|| raw.strip_prefix('-'))
            .unwrap_or(raw);
        if let Some((name, value)) = option.split_once('=') {
            self.inline_value = Some(value);
            Some(name)
        } else {
            Some(option)
        }
    }

    pub fn is_option(&self) -> bool {
        self.option
    }

    pub fn value(&mut self, flag: &str) -> Result<&'a str, String> {
        if let Some(value) = self.inline_value.take() {
            return Ok(value);
        }
        let value = self
            .arguments
            .get(self.index)
            .ok_or_else(|| format!("{flag} requires a value"))?;
        self.index += 1;
        Ok(value)
    }

    pub fn boolean(&mut self, flag: &str) -> Result<bool, String> {
        let Some(value) = self.inline_value.take() else {
            return Ok(true);
        };
        match value {
            "1" | "t" | "T" | "true" | "TRUE" | "True" => Ok(true),
            "0" | "f" | "F" | "false" | "FALSE" | "False" => Ok(false),
            _ => Err(format!("invalid value {value:?} for {flag}")),
        }
    }

    pub fn finish(&self) -> Result<(), String> {
        if let Some(value) = self.arguments.get(self.index) {
            Err(format!("unexpected argument {value:?}"))
        } else {
            Ok(())
        }
    }
}

pub fn parse_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} requires a non-negative integer"))
}

pub fn parse_duration(flag: &str, value: &str) -> Result<std::time::Duration, String> {
    humantime::parse_duration(value).map_err(|error| format!("invalid {flag}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_go_style_flag_forms() {
        let arguments = vec![
            "-repo=one".to_owned(),
            "--languages".to_owned(),
            "python".to_owned(),
            "--dry-run=false".to_owned(),
        ];
        let mut parser = Parser::new(&arguments);

        assert_eq!(parser.next(), Some("repo"));
        assert_eq!(parser.value("--repo").unwrap(), "one");
        assert_eq!(parser.next(), Some("languages"));
        assert_eq!(parser.value("--languages").unwrap(), "python");
        assert_eq!(parser.next(), Some("dry-run"));
        assert!(!parser.boolean("--dry-run").unwrap());
        assert_eq!(parser.next(), None);
        parser.finish().unwrap();
    }
}
