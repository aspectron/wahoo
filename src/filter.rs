use crate::prelude::*;

pub struct Filter {
    matchers : Vec<GlobMatcher>
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            matchers: Vec::new()
        }
    }
}

impl Filter {
    pub fn new(globs: &[&str]) -> Filter {
        let matchers = globs.iter().map(|glob|{
            Glob::new(glob).expect(&format!("Error compiling glob: {}", glob)).compile_matcher()
        }).collect::<Vec<_>>();

        Filter {
            matchers
        }
    }

    pub fn is_match(&self, text : &str) -> bool {
        self.matchers
            .iter()
            .find(|filter| filter.is_match(text))
            .is_some()
    }
}
