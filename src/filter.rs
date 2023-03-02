use crate::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use tera::Context;
use walkdir::WalkDir;

#[derive(Default, Debug)]
pub struct Filter {
    matchers: Vec<GlobMatcher>,
}

impl Filter {
    pub fn new(globs: &[&str]) -> Filter {
        let matchers = globs
            .iter()
            .map(|glob| {
                Glob::new(glob)
                    .unwrap_or_else(|_| panic!("Error compiling glob: {glob}"))
                    .compile_matcher()
            })
            .collect::<Vec<_>>();

        Filter { matchers }
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.matchers.iter().any(|filter| filter.is_match(text))
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

pub fn markdown(template_folder: &Path, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let mut content = None;
    if let Some(c) = args.get("content") {
        if let Some(c) = c.as_str() {
            let c = c.replace("\\n", "\r\n");
            content = Some(c)
        }
    } else if let Some(file) = args.get("file") {
        if let Some(file) = file.as_str() {
            //let complete_path = template_folder.join(file);
            content = match std::fs::read_to_string(template_folder.join(file)) {
                Ok(c) => Some(c),
                Err(e) => {
                    log_warn!("Markdown", "Unable to read file `{}`: {}", file, e);
                    // return Err(
                    //     format!("Unable to read file {:?}, error: {}", file.to_str(), e).into(),
                    // );
                    Some("".to_string())
                }
            };
        }
    } else {
        return Err("Use {{ markdown(content=\"# title\\ntest contents\") | safe}} or {{ markdown(file=\"path/to/file\") | safe}}".into());
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

fn read_md_file_impl(path: &Path, root_folder:&str, open_in_new_window: bool) -> tera::Result<Value> {
    let value = match std::fs::read_to_string(path) {
        Ok(str) => {
            let toml_text = parse_toml_from_markdown(&str);
            //println!("toml_text: {:?}", toml_text);
            let file_name = path.file_name().unwrap().to_str().unwrap();
            let html = markdown_to_html(&str, open_in_new_window);
            let toml = if let Some(toml_text) = toml_text {
                let toml: Value = match toml::from_str(&toml_text) {
                    Ok(value) => value,
                    Err(err) => {
                        return Err(
                            format!("Error parsing: {}, error: {err}", path.display()).into()
                        );
                    }
                };
                //Value::from(toml.serialize())
                toml
            } else {
                Value::Null
            };
            let html = Value::String(html);
            serde_json::json!({
                "file_name" : file_name,
                "path" : path.to_str().unwrap().replace(root_folder, ""),
                "file": file_name.replace(".md", ""),
                "toml" : toml,
                "html" : html
            })
        }
        Err(e) => {
            return Err(format!("Unable to read file {:?}, error: {}", path.to_str(), e).into());
        }
    };

    Ok(value)
}

pub fn read_md_file(template_folder: &Path, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let file_path = if let Some(file) = args.get("file") {
        if let Some(file) = file.as_str() {
            file
        } else {
            return Err("read_md_file: Unable to parse file path from arguments".into());
        }
    } else {
        return Err("Use {% read_md_file(file=\"path/to/file\") %}".into());
    };

    let mut path = template_folder.join(file_path);
    if path.exists() {
        path = path.canonicalize()?;
    } else {
        return Err(format!("read_md_file: path dont exists: {path:?}").into());
    }

    let mut open_in_new_window = true;
    if let Some(new_window) = args.get("external_links") {
        if let Some(new_window) = new_window.as_bool() {
            open_in_new_window = new_window;
        }
    }

    let root_folder = template_folder.parent().unwrap().parent().unwrap().to_str().unwrap();

    read_md_file_impl(&path, root_folder, open_in_new_window)
}
pub fn read_md_files(template_folder: &Path, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let dir_path = if let Some(file) = args.get("dir") {
        if let Some(file) = file.as_str() {
            file
        } else {
            return Err("read_md_files: Unable to parse directory path from arguments".into());
        }
    } else {
        return Err("Use {% read_md_files(dir=\"path/to/directory\") %}".into());
    };

    let mut path = template_folder.join(dir_path);
    if path.exists() {
        path = path.canonicalize()?;
    } else {
        return Err(format!("read_md_files: path dont exists: {path:?}").into());
    }

    let list = WalkDir::new(path)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();

            let str = path.to_str().unwrap();
            if !str.ends_with(".md") || is_hidden(path) {
                return None;
            }

            Some(Path::new(path.to_str().unwrap()).to_path_buf())
        });

    let mut open_in_new_window = true;
    if let Some(new_window) = args.get("external_links") {
        if let Some(new_window) = new_window.as_bool() {
            open_in_new_window = new_window;
        }
    }
    let mut md_list = Vec::new();
    let root_folder = template_folder.parent().unwrap().parent().unwrap().to_str().unwrap();
    for path in list {
        md_list.push(read_md_file_impl(&path, root_folder, open_in_new_window)?);
    }
    //println!("###### md_list : {:?}", md_list);
    Ok(Value::Array(md_list))
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

    fn create_tera(&self) -> tera::Result<tera::Tera> {
        let mut tera = match tera::Tera::new(&self.dir) {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}, glob:{}", e, self.dir);
                return Err(e);
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

        Ok(tera)
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

        let mut tera = self.create_tera()?;

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
            let path = self.project_folder.join("templates").join(template);
            if path.exists() {
                let path = path.canonicalize()?;
                //println!("path: {:?}", path);
                tera.add_template_file(path, Some(template))?;
            } else {
                return Err(format!("Template not found: {template}").into());
            }
        }

        let mut context = self.context.clone();
        context.extend(Context::from_serialize(args)?);

        //let context = self.context.clone();

        //tera.add_template_file();
        /*
        println!("include_file: {:?},\ntpl: {},\ncontext: {:#?}",
            value.as_str().unwrap(),
            template,
            context.clone().into_json()
        );
        */

        if rendering_fallback {
            log_trace!(
                "Rendering",
                "include_file={}, fallback={}",
                style(value.as_str().unwrap()).cyan(),
                style(template).blue()
            );
        } else {
            log_trace!(
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
