use crate::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use tera::Context;
use walkdir::WalkDir;

pub struct Filter {
    matchers: Vec<GlobMatcher>,
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            matchers: Vec::new(),
        }
    }
}

impl Filter {
    pub fn new(globs: &[&str]) -> Filter {
        let matchers = globs
            .iter()
            .map(|glob| {
                Glob::new(glob)
                    .expect(&format!("Error compiling glob: {}", glob))
                    .compile_matcher()
            })
            .collect::<Vec<_>>();

        Filter { matchers }
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.matchers
            .iter()
            .find(|filter| filter.is_match(text))
            .is_some()
    }
}

pub struct Log {}

impl tera::Filter for Log {
    fn filter(&self, value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
        let default_value = Value::String("Log:".to_string());
        let title = args.get("title").unwrap_or(&default_value);
        println!("\n{}: {:?}", title.as_str().unwrap(), value);
        Ok(value.clone())
    }
}

pub struct SortObject {}

impl tera::Filter for SortObject {
    fn filter(
        &self,
        value: &Value,
        _args: &std::collections::HashMap<String, Value>,
    ) -> tera::Result<Value> {
        let mut result: Vec<Value> = Vec::new();
        if value.is_object() {
            for (key, value) in value.as_object().unwrap() {
                let mut object = value.as_object().unwrap().clone();
                object.insert("key".to_string(), Value::String(key.to_string()));
                result.push(Value::Object(object));
            }

            result.sort_by(|a, b| {
                let empty = serde_json::json!(0);
                let a_sort = a.get("sort-index").unwrap_or(&empty).as_i64().unwrap();
                let b_sort = b.get("sort-index").unwrap_or(&empty).as_i64().unwrap();
                a_sort.cmp(&b_sort)
            });
        }

        Ok(Value::Array(result))
    }
}

pub struct Markdown {}

impl tera::Filter for Markdown {
    fn filter(&self, value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
        if !value.is_string() {
            return Ok(value.clone());
        }
        let mut open_in_new_window = true;
        if let Some(new_window) = args.get("external_links") {
            if let Some(new_window) = new_window.as_bool() {
                open_in_new_window = new_window;
            }
        }

        let str = value.as_str().unwrap();
        let result = markdown_to_html(str, open_in_new_window);
        Ok(Value::String(result))
    }
}

pub fn markdown(project_folder: &PathBuf, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let mut content = None;
    if let Some(c) = args.get("content") {
        if let Some(c) = c.as_str() {
            let c = c.replace("\\n", "\r\n");
            content = Some(c)
        }
    } else if let Some(file) = args.get("file") {
        if let Some(file) = file.as_str() {
            let file = project_folder.join(file);
            content = match std::fs::read_to_string(&file) {
                Ok(c) => Some(c),
                Err(e) => {
                    return Err(
                        format!("Unable to read file {:?}, error: {}", file.to_str(), e).into(),
                    );
                }
            };
        }
    } else {
        return Err("Use {% markdown(content=\"# title\ntest contents\") %} or {% markdown(file=\"path/to/file\") %}".into());
    }

    let mut open_in_new_window = true;
    if let Some(new_window) = args.get("external_links") {
        if let Some(new_window) = new_window.as_bool() {
            open_in_new_window = new_window;
        }
    }

    if let Some(str) = content {
        let result = markdown_to_html(&str, open_in_new_window);
        Ok(Value::String(result))
    } else {
        Err("Use {% markdown(content=\"# title\ntest contents\") %} or {% markdown(file=\"path/to/file\") %}".into())
    }
}

pub fn read_md_files(
    project_folder: &PathBuf,
    args: &HashMap<String, Value>,
) -> tera::Result<Value> {
    let mut dir_path = None;
    if let Some(file) = args.get("dir") {
        if let Some(file) = file.as_str() {
            dir_path = Some(project_folder.join(file));
        }
    } else {
        return Err("Use {% parse_md_files(dir=\"path/to/directory\") %}".into());
    }
    if let Some(dir_path) = dir_path {
        let list = WalkDir::new(&project_folder.join(dir_path))
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let relative = path.strip_prefix(&project_folder).unwrap();

                let relative_str = relative.to_str().unwrap();
                if !relative_str.ends_with(".md") || is_hidden(relative) {
                    return None;
                }

                Some(Path::new(path.to_str().unwrap()).to_path_buf())
            });
        //println!("###### list : {:?}", list);

        let mut open_in_new_window = true;
        if let Some(new_window) = args.get("external_links") {
            if let Some(new_window) = new_window.as_bool() {
                open_in_new_window = new_window;
            }
        }
        let mut md_list = Vec::new();
        for path in list {
            let value = match std::fs::read_to_string(&path) {
                Ok(str) => {
                    let toml_text = parse_toml_from_markdown(&str);
                    //println!("toml_text: {:?}", toml_text);
                    let html = markdown_to_html(&str, open_in_new_window);
                    let toml = if let Some(toml_text) = toml_text {
                        let toml: Value = match toml::from_str(&toml_text) {
                            Ok(value) => value,
                            Err(err) => {
                                return Err(format!(
                                    "Error parsing: {}, error: {err}",
                                    path.display()
                                )
                                .into());
                            }
                        };
                        //Value::from(toml.serialize())
                        toml
                    } else {
                        Value::Null
                    };
                    let html = Value::String(html);
                    serde_json::json!({
                        "toml" : toml,
                        "html" : html
                    })
                }
                Err(e) => {
                    return Err(
                        format!("Unable to read file {:?}, error: {}", path.to_str(), e).into(),
                    );
                }
            };

            md_list.push(value);
        }
        //println!("###### md_list : {:?}", md_list);
        Ok(Value::Array(md_list))
    } else {
        Err("parse_md_files: Unable to parse directory path from arguments".into())
    }
}

#[derive(Clone)]
pub struct IncludeFile {
    pub project_folder: PathBuf,
    pub dir: String,
    pub context: tera::Context,
}

impl IncludeFile {
    pub fn new(project_folder: PathBuf, dir: &str, context: tera::Context) -> Self {
        Self {
            project_folder,
            dir: dir.to_string(),
            context,
        }
    }

    fn create_tera(&self) -> tera::Tera {
        let mut tera = match tera::Tera::new(&self.dir) {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}, glob:{}", e, self.dir);
                ::std::process::exit(1);
            }
        };

        let log = Log {};
        let sort_object = SortObject {};
        let markdown_filter = Markdown {};
        let include_file = self.clone();

        let project_folder = self.project_folder.join("templates");

        tera.register_filter("sort_object", sort_object);
        tera.register_filter("markdown", markdown_filter);
        tera.register_filter("include_file", include_file);
        tera.register_filter("log", log);
        tera.register_function(
            "markdown",
            move |args: &HashMap<String, Value>| -> tera::Result<Value> {
                let value = markdown(&project_folder, args)?;
                Ok(value)
            },
        );

        tera
    }
}

impl tera::Filter for IncludeFile {
    fn filter(
        &self,
        value: &Value,
        args: &std::collections::HashMap<String, Value>,
    ) -> tera::Result<Value> {
        //println!("IncludeFile: file: {:?}, args: {:?}", value, args);
        if !value.is_string() {
            return Ok(value.clone());
        }
        let mut template = value.as_str().unwrap();

        let tera = self.create_tera();

        let templates: Vec<&str> = tera.get_template_names().collect();
        let mut rendering_fallback = false;
        if !templates.contains(&template) {
            if let Some(default) = args.get("default") {
                if let Some(path) = default.as_str() {
                    template = path;
                    rendering_fallback = true;
                }
            }
        }

        if !templates.contains(&template) {
            return Err(format!("Template not found: {}", template).into());
        }

        let mut context = Context::from_serialize(args)?;
        context.insert("super".to_string(), &self.context.clone().into_json());

        //tera.add_template_file();
        /*
        println!("include_file: {:?},\ntpl: {},\ncontext: {:#?}",
            value.as_str().unwrap(),
            template,
            context.clone().into_json()
        );
        */

        if rendering_fallback {
            log_info!(
                "Rendering",
                "include_file={}, fallback={}",
                style(value.as_str().unwrap()).cyan(),
                style(template).blue()
            );
        } else {
            log_info!(
                "Rendering",
                "include_file={}",
                style(value.as_str().unwrap()).blue()
            );
        }

        match tera.render(template, &context) {
            Ok(result) => Ok(Value::String(result)),
            Err(e) => {
                //log_error!("IncludeFile::render error: {:?}", e);
                Err(e)
            }
        }
    }
}
