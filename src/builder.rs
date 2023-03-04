use crate::prelude::*;
use std::sync::Mutex;
use std::time::Instant;
use tera::Filter as TeraFilter;
use walkdir::WalkDir;
use workflow_i18n::Dict;

const SERVER_STUBS_TPL: &str = include_str!("./server-stubs.html");

static mut SERVER_STUBS: Option<Mutex<String>> = None;

// fn server_stubs(tpl : &Option<HashMap<String,String>>) -> String {
fn server_stubs(settings: &Option<Settings>) -> String {
    let text = if let Some(text) = unsafe { &SERVER_STUBS } {
        text.lock().unwrap().clone()
    } else {
        let mut text = SERVER_STUBS_TPL.to_string();

        text = if let Some(Settings {
            scroll_element: Some(scroll_element),
            ..
        }) = settings
        {
            let code = if let Some(id) = &scroll_element.id {
                format!("return document.getElementById(\"{id}\");")
            } else if let Some(class) = &scroll_element.class {
                format!("return document.getElementsByClassName(\"{class}\")[0];")
            } else if let Some(tag) = &scroll_element.tag {
                format!("return document.getElementsByTagName(\"{tag}\")[0];")
            } else {
                "return document.getElementById(\"main\");".to_string()
            };

            text.replace("return document.getElementById(\"main\");", &code)
        } else {
            text
        };

        unsafe { SERVER_STUBS = Some(Mutex::new(text.clone())) };
        text
    };

    text
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Language {
    locale: String,
    name: String,
}

#[derive(Clone)]
pub struct Builder {
    ctx: Arc<Context>,
    sink: Sink,
    i18n_dict: Arc<Dict>,
}

pub struct SectionInfo {
    name: String,
    template_file: String,
    files: Vec<String>,
}

impl Builder {
    pub fn new(ctx: Arc<Context>) -> Builder {
        Builder {
            ctx,
            sink: Sink::default(),
            i18n_dict: Arc::new(Dict::default()),
        }
    }

    pub fn new_with_sink(ctx: Arc<Context>, sink: Sink) -> Builder {
        Builder {
            ctx,
            sink,
            i18n_dict: Arc::new(Dict::default()),
        }
    }

    /// Migrate non-template files into the target directory
    pub async fn migrate(&self, include: &Filter, exclude: &Filter) -> Result<()> {
        let list = WalkDir::new(&self.ctx.src_folder)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let relative = path.strip_prefix(&self.ctx.src_folder).unwrap();

                let relative_str = relative.to_str().unwrap();
                if relative_str.is_empty() || is_hidden(relative) {
                    return None;
                }

                if include.is_match(relative_str) || !path.is_file() {
                    None
                } else if exclude.is_match(relative_str) {
                    log_trace!(
                        "Migrate",
                        "{} `{}`",
                        style("ignore:").yellow(),
                        path.display()
                    );
                    None
                } else {
                    Some(Path::new(relative).to_path_buf())
                }
            });

        let list: Vec<_> = list.collect();

        self.sink.migrate(&self.ctx, &list)?;

        // let mut folders = HashSet::new();
        // for path in list.iter() {
        //     if let Some(folder) = path.parent() {
        //         if folder.to_string_lossy().len() != 0 {
        //             folders.insert(folder.to_path_buf());
        //         }
        //     }
        // }

        // for folder in folders {
        //     log_trace!("Migrate", "folder `{}`", folder.display());
        //     std::fs::create_dir_all(self.ctx.site_folder.join(folder))?;
        // }

        // for file in list {
        //     let to_file = self.ctx.site_folder.join(&file);
        //     // println!("+{}",file.display());
        //     log_trace!(
        //         "Migrate",
        //         "{} `{}` to `{}`",
        //         style("migrate:").cyan(),
        //         file.display(),
        //         to_file.display()
        //     );
        //     std::fs::copy(self.ctx.src_folder.join(&file), to_file)?;
        // }

        Ok(())
    }

    fn save_file(&self, content: &str, template: &str, language: Option<&String>) -> Result<()> {
        let target_file = if let Some(language) = language {
            self.ctx.site_folder.join(language).join(template)
        } else {
            self.ctx.site_folder.join(template)
        };
        let folder = target_file.parent().unwrap();
        if !std::path::Path::new(folder).exists() {
            std::fs::create_dir_all(folder)?;
        }
        log_trace!("Render", "{} `{}`", style("render:").cyan(), template);
        std::fs::write(target_file, content)?;
        Ok(())
    }

    fn render_template(
        &self,
        tera: &tera::Tera,
        template: &str,
        context: &mut tera::Context,
        language: &Language,
        url_prefix: &str,
    ) -> Result<String> {
        use std::error::Error;

        context.insert("url_prefix", url_prefix);
        context.insert("locale", &language.locale);
        context.insert("selected_language", &language);

        log_trace!(
            "Rendering",
            "{}",
            style(format!("{url_prefix}{template}")).blue()
        );

        match tera.render(template, context) {
            Ok(mut s) => {
                if self.ctx.options.server {
                    s += &server_stubs(&self.ctx.manifest.settings);
                }
                Ok(s)
            }
            Err(err) => {
                let mut error_string = String::new();
                let mut cause = err.source();
                while let Some(err) = cause {
                    log_error!("{}", err);
                    error_string += &format!("<code>{err}</code>\n");
                    cause = err.source();
                }

                // Err(e.into())
                if self.ctx.options.server {
                    error_string += &server_stubs(&self.ctx.manifest.settings);
                    Ok(error_string)
                } else {
                    Ok(error_string)
                }
            }
        }
    }

    /// Render templates into the target directory
    pub async fn render(
        &self,
        glob: &str,
        exclude: &Filter,
        settings: &Settings,
        sections: HashMap<String, SectionInfo>,
    ) -> Result<()> {
        let project_folder = self.ctx.src_folder.clone();
        let dir = self.ctx.src_folder.join(glob);
        let dir = dir.to_str().unwrap();
        let mut tera = match tera::Tera::new(dir) {
            Ok(t) => t,
            Err(err) => {
                log_error!("Parsing error(s): {err}, glob:{glob}");
                // return Err(err.into());
                return Ok(());
            }
        };

        let mut context = tera::Context::from_serialize(&self.ctx.manifest.toml)?;
        if let Some(sections) = &self.ctx.sections() {
            context.insert("sections", sections);
        }

        let sort_object = SortObject {};
        let markdown_filter = Markdown {};

        let include_file = IncludeFile::new(project_folder.clone(), dir, context.clone());

        let templates_folder = project_folder.join("templates");
        let log = Log {};

        tera.register_filter("sort_object", sort_object);
        tera.register_filter("markdown", markdown_filter);
        tera.register_filter("include_file", include_file.clone());
        tera.register_filter("log", log);

        let get_arg = |name: &str, args: &HashMap<String, tera::Value>| -> tera::Result<String> {
            let value = if let Some(value) = args.get(name) {
                if let Some(value) = value.as_str() {
                    value
                } else {
                    return Err(format!("invalid `{name}` ({value:?}) argument").into());
                }
            } else {
                return Err(format!("`{name}` argument is missing").into());
            };

            Ok(value.to_string())
        };

        tera.register_function(
            "include_file",
            move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
                let file = get_arg("file", args)?;

                let value = include_file.filter(&file.into(), args)?;
                Ok(value)
            },
        );
        let templates_folder_ = templates_folder.clone();
        tera.register_function(
            "markdown",
            move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
                let value = markdown(&templates_folder_, args)?;
                Ok(value)
            },
        );
        let templates_folder_ = templates_folder.clone();
        tera.register_function(
            "save_file",
            move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
                let file = get_arg("file", args)?;
                let content = get_arg("content", args)?;
                let path = templates_folder_.join(file);

                log_trace!("SaveFile", "{} `{:?}`", style("save_file:").cyan(), path);
                std::fs::write(path, content)?;
                Ok(tera::Value::Bool(true))
            },
        );
        let templates_folder_ = templates_folder.clone();
        tera.register_function(
            "read_md_files",
            move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
                let value = read_md_files(&templates_folder_, args)?;
                Ok(value)
            },
        );
        let templates_folder_ = templates_folder.clone();
        tera.register_function(
            "read_md_file",
            move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
                let value = read_md_file(&templates_folder_, args)?;
                Ok(value)
            },
        );

        log_trace!("Render", "processing folders");

        /*
        let mut folders = HashSet::new();
        if sections.is_none() {
            for template in tera.get_template_names() {
                // let target_file = self.ctx.target_folder.join(template);
                let folder = Path::new(&template);
                if let Some(parent) = folder.parent() {
                    let text = parent.to_str().unwrap();
                    if exclude.is_match(text) {
                        continue;
                    }
                    if is_hidden(text) {
                        continue;
                    }
                    if parent.to_string_lossy().len() != 0 {
                        folders.insert(parent.to_path_buf());
                    }
                }
            }
        }
        */

        let mut language_list = Vec::new();
        let info = if let Some(languages) = &settings.languages {
            if languages.is_empty() {
                return Err(
                    "Please provide any language or disable `settings.languages` from `wahoo.toml`"
                        .into(),
                );
            }
            let mut list = Vec::new();

            for locale in languages {
                let url_prefix = format!("/{locale}/");
                let name = match self.i18n_dict.language(locale) {
                    Ok(name) => {
                        if let Some(name) = name {
                            name.to_string()
                        } else {
                            return Err(format!("Unknown language: {locale}").into());
                        }
                    }
                    Err(e) => {
                        return Err(format!(
                            "Could not find language ({locale}) in workflow_i18n dict, error:{e}"
                        )
                        .into());
                    }
                };

                let lang = Language {
                    name,
                    locale: locale.clone(),
                };
                language_list.push(lang.clone());
                list.push((url_prefix, Some(locale.clone()), lang))
            }

            self.render_redirecting_index_page(&mut tera, &mut context)
                .await?;

            list
        } else {
            vec![(
                "/".to_string(),
                None,
                Language {
                    name: "English".to_string(),
                    locale: "en".to_string(),
                },
            )]
        };

        context.insert("languages", &language_list);

        /*
        for (_, language, _) in &info {
            let path = if let Some(language) = language {
                self.ctx.site_folder.join(language)
            } else {
                self.ctx.site_folder.clone()
            };

            for folder in &folders {
                log_trace!(
                    "Folders",
                    "{} `{}`",
                    style("creating:").cyan(),
                    folder.display()
                );
                std::fs::create_dir_all(path.join(folder))?;
            }
        }
        */

        struct RenderFile {
            #[allow(clippy::type_complexity)]
            callback: Arc<
                Mutex<
                    dyn FnMut(&HashMap<String, tera::Value>) -> tera::Result<tera::Value>
                        + Sync
                        + Send,
                >,
            >,
        }

        impl tera::Function for RenderFile {
            fn call(&self, args: &HashMap<String, tera::Value>) -> tera::Result<tera::Value> {
                if let Ok(f) = self.callback.lock().as_mut() {
                    (f)(args)
                } else {
                    Ok(tera::Value::Null)
                }
            }
        }

        let info_ = info.clone();
        let this = self.clone();
        let tera_ = tera.clone();
        let mut context_ = context.clone();

        let mut render_file =
            move |template: String, destination: String, args: &HashMap<String, tera::Value>| {
                log_trace!(
                    "RenderFile",
                    "{} {} => {}",
                    style("render_file:").cyan(),
                    template,
                    destination
                );
                for (url_prefix, folder, language) in &info_ {
                    context_.extend(tera::Context::from_serialize(args).unwrap());
                    let content = this.render_template(
                        &tera_,
                        &template,
                        &mut context_,
                        language,
                        url_prefix,
                    );

                    let this_ = this.clone();
                    if let Ok(content) = content {
                        let template_ = template.clone();
                        let destination_ = destination.clone();
                        let folder_ = folder.clone();
                        this_
                            .save_file(&content, &destination_, folder_.as_ref())
                            .map_err(|err| {
                                log_warn!(
                                    "RenderFile",
                                    "Unable to render template: {template_}, error: {err:?}"
                                );
                            })
                            .ok();
                    } else {
                        log_warn!(
                            "RenderFile",
                            "Unable to render template: {template}, error: {content:?}"
                        );
                    }
                }
            };

        let mut render_file_ = render_file.clone();

        tera.register_function(
            "render_file",
            RenderFile {
                callback: Arc::new(Mutex::new(
                    move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
                        let template = get_arg("file", args)?;
                        let destination = get_arg("dest", args)?;

                        render_file_(template, destination, args);
                        Ok(tera::Value::Bool(true))
                    },
                )),
            },
        );

        log_trace!("Render", "rendering");

        let md_tpl_file = &settings.markdown.clone().unwrap_or(".md.html".to_string());
        let default_dm_template_path = templates_folder.join(md_tpl_file);
        let mut default_dm_template = None;
        if default_dm_template_path.exists() {
            default_dm_template = Some(md_tpl_file);
        }

        for template in tera.get_template_names() {
            let root_folder = match root_folder(template) {
                Some(folder) => folder,
                None => {
                    continue;
                }
            };

            let mut destination = template.to_string();

            let section = sections.get(&root_folder);

            //println!("root_folder: {root_folder} template: {template}");
            if let Some(section) = section {
                if section.template_file.ends_with(template) {
                    let sep = format!("/{}/", section.name);
                    for file in &section.files {
                        context.insert("section_file", file);
                        let mut parts = file.split(&sep);
                        parts.next();
                        let destination = match parts.next() {
                            Some(value) => PathBuf::from(&format!("{}/{}", section.name, value))
                                .with_extension("html"),
                            None => continue,
                        };
                        //println!("destination: {destination:?}");
                        //println!("section_file: {file:?}");

                        for (url_prefix, folder, language) in &info {
                            let content = self.render_template(
                                &tera,
                                template,
                                &mut context,
                                language,
                                url_prefix,
                            )?;
                            self.save_file(
                                &content,
                                destination.to_str().unwrap(),
                                folder.as_ref(),
                            )?;
                        }
                    }
                    continue;
                }
                if is_file_hidden(template) {
                    continue;
                }
                destination =
                    template.replace(&format!("{root_folder}/"), &format!("{}/", section.name));
                //println!("destination: {destination}");
            } else if is_hidden(template) {
                continue;
            }

            if exclude.is_match(template) {
                log_trace!("Render", "{} `{}`", style("ignore:").yellow(), template);
                continue;
            }

            if template.ends_with(".md") {
                if default_dm_template.is_none() {
                    continue;
                }
                let tpl_path = Path::new(template);
                let md_template = default_dm_template.as_ref().unwrap();
                let destination = Path::new(&destination)
                    .with_extension("html")
                    .to_str()
                    .unwrap()
                    .to_string();
                let file_name = tpl_path.file_name().unwrap().to_str().unwrap();
                let mut args: HashMap<String, tera::Value> = HashMap::new();
                args.insert("file_name".to_string(), file_name.into());
                args.insert("file_path".to_string(), template.into());
                args.insert(
                    "file_id".to_string(),
                    template.replace('/', "-").replace(".md", "").into(),
                );
                args.insert("file".to_string(), file_name.replace(".md", "").into());

                render_file(md_template.to_string(), destination, &args);
            } else {
                for (url_prefix, folder, language) in &info {
                    let content =
                        self.render_template(&tera, template, &mut context, language, url_prefix)?;
                    self.save_file(&content, &destination, folder.as_ref())?;
                }
            }
        }

        //println!("context: {:#?}", context.into_json());

        if self.ctx.options.server {
            let update_json_file = self.ctx.site_folder.join("__wahoo.json");
            std::fs::write(update_json_file, "{}").ok();
        }

        Ok(())
    }

    pub async fn execute(&self) -> Result<()> {
        // if !self.options.serve
        self.sink.init(&self.ctx).await?;
        // if sink.is_none() {
        //     self.ctx.clean().await?;
        // }
        // self.ctx.ensure_folders().await?;

        let glob = "templates/**/*{.html,.md,.js,.raw}";
        let include = Filter::new(&[glob]);

        let settings = self.ctx.settings();

        let mut exclude_list = if let Some(ignore) = &settings.ignore {
            let mut list = ignore.iter().map(|s| s.as_str()).collect::<Vec<_>>();
            list.push("__INDEX__.html");

            list
        } else {
            vec!["__INDEX__.html"]
        };

        let mut exclude = Filter::new(&exclude_list);
        let mut section_infos = HashMap::new();
        // render sections
        if let Some(sections) = &self.ctx.manifest.sections {
            log_trace!("Render", "loading sections");

            let mut section_exclude_list = vec![];
            for (name, section) in sections {
                let section_settings = match &section.settings {
                    Some(value) => value,
                    None => continue,
                };

                let enumerate = match section_settings.enumerate {
                    Some(value) => value,
                    None => continue,
                };

                if !enumerate {
                    continue;
                }

                let template = match &section_settings.template {
                    Some(value) => value,
                    None => continue,
                };

                let index_file = match &section_settings.index {
                    Some(value) => value,
                    None => continue,
                };

                if !template.ends_with(".html") || !index_file.ends_with(".html") {
                    continue;
                }

                let folder = match &section_settings.folder {
                    Some(value) => value,
                    None => continue,
                };

                let files = WalkDir::new(self.ctx.project_folder.join(folder))
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| {
                        let path = entry.path();
                        let relative = path.strip_prefix(&self.ctx.project_folder).unwrap();

                        let r = relative.to_str().unwrap();
                        if !(r.ends_with(".md") || r.ends_with(".html")) || is_hidden(relative) {
                            return None;
                        }

                        //let _is_dir = entry.file_type().is_dir();

                        Some(format!("../../{}", relative.to_str().unwrap()))
                    })
                    .collect::<Vec<String>>();

                let folder = root_folder(template).unwrap();
                let section_info = SectionInfo {
                    name: name.clone(),
                    template_file: template.clone(),
                    files,
                };

                section_exclude_list.push(template);
                section_infos.insert(folder, section_info);
            }

            for a in section_exclude_list {
                exclude_list.push(a);
            }

            exclude = Filter::new(&exclude_list);

            //println!("exclude: {exclude:#?}");
        }

        let render_start = Instant::now();
        log_trace!("Migrate", "migrating files");
        self.migrate(&include, &exclude).await?;
        log_trace!("Render", "loading templates");
        self.render(glob, &exclude, &settings, section_infos)
            .await?;

        let duration = render_start.elapsed();
        log_info!(
            "Build",
            "rendering complete in {} msec",
            duration.as_millis()
        );

        let package_json = self.ctx.site_folder.join("package.json");
        let node_modules = self.ctx.site_folder.join("node_modules");
        if package_json.is_file() && !node_modules.is_dir() {
            log_info!("NPM","detected `package.json`; installing ... ");
            println!();
            cmd!("npm", "install").dir(&self.ctx.site_folder).run()?;
            println!();
        }

        Ok(())
    }

    pub async fn render_redirecting_index_page(
        &self,
        tera: &mut tera::Tera,
        context: &mut tera::Context,
    ) -> Result<()> {
        tera.add_raw_template(
            "__INDEX__.html",
            "<!DOCTYPE html><html lang=\"en-gb\"><head><script>window.location.href=\"/en/index.html\";</script></head><body>Please wait. Redirecting...</body></html>"
        )?;

        let url_prefix = "/en/".to_string();

        let language = Language {
            name: "English".to_string(),
            locale: "en".to_string(),
        };

        let content =
            self.render_template(tera, "__INDEX__.html", context, &language, &url_prefix)?;
        self.save_file(&content, "index.html", None)?;

        Ok(())
    }
}
