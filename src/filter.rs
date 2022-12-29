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


pub struct SortObject{}

impl tera::Filter for SortObject{
    fn filter(&self, 
        value: &serde_json::Value,
        _args: &std::collections::HashMap<String, serde_json::Value>
    ) -> tera::Result<serde_json::Value> {
        let mut result:Vec<serde_json::Value> = Vec::new();
        if value.is_object(){
            for (key, value) in value.as_object().unwrap(){
                let mut object = value.as_object().unwrap().clone();
                object.insert("key".to_string(), serde_json::Value::String(key.to_string()));
                result.push(serde_json::Value::Object(object));
            }

            result.sort_by(|a, b| {
                let empty = serde_json::json!(0);
                let a_sort = a.get("sort").unwrap_or(&empty).as_i64().unwrap();
                let b_sort = b.get("sort").unwrap_or(&empty).as_i64().unwrap();
                a_sort.cmp(&b_sort)
            });
        }

        Ok(serde_json::Value::Array(result))
    }
}
