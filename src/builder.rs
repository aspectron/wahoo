use crate::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};
use walkdir::WalkDir;
use workflow_i18n::Dict;
use tera::Filter as TeraFilter;

const SERVER_STUBS: &str = include_str!("./server-stubs.html");

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Language {
    locale: String,
    name: String,
}

pub struct Builder {
    ctx: Arc<Context>,
    i18n_dict: Dict,
}

pub struct SectionInfo{
    name: String,
    template: String,
    files: Vec<String>,
}

impl Builder {
    pub fn new(ctx: Arc<Context>) -> Builder {
        Builder {
            ctx,
            i18n_dict: Dict::default(),
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

        let mut folders = HashSet::new();
        let list: Vec<_> = list.collect();
        for path in list.iter() {
            if let Some(folder) = path.parent() {
                if folder.to_string_lossy().len() != 0 {
                    folders.insert(folder.to_path_buf());
                }
            }
        }

        for folder in folders {
            log_trace!("Migrate", "folder `{}`", folder.display());
            std::fs::create_dir_all(self.ctx.site_folder.join(folder))?;
        }

        for file in list {
            let to_file = self.ctx.site_folder.join(&file);
            // println!("+{}",file.display());
            log_trace!(
                "Migrate",
                "{} `{}` to `{}`",
                style("migrate:").cyan(),
                file.display(),
                to_file.display()
            );
            std::fs::copy(self.ctx.src_folder.join(&file), to_file)?;
        }

        Ok(())
    }

    async fn save_file(
        &self,
        content: &str,
        template: &str,
        language: Option<&String>,
    ) -> Result<()> {
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
        fs::write(target_file, content).await?;
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
                    s += SERVER_STUBS;
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
                    error_string += SERVER_STUBS;
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
        section: Option<SectionInfo>,
    ) -> Result<()> {
        let project_folder = self.ctx.src_folder.clone();
        let dir = self.ctx.src_folder.join(glob);
        let dir = dir.to_str().unwrap();
        let mut tera = match tera::Tera::new(dir) {
            Ok(t) => t,
            Err(err) => {
                println!("Parsing error(s): {err}, glob:{glob}");
                ::std::process::exit(1);
            }
        };

        let mut context = tera::Context::from_serialize(&self.ctx.manifest.toml)?;
        if let Some(sections) = &self.ctx.sections() {
            context.insert("sections", sections);
        }
        //println!("self.ctx.manifest.sections: {:#?}", self.ctx.manifest.sections);

        let sort_object = SortObject {};
        let markdown_filter = Markdown {};

        let include_file = IncludeFile::new(project_folder.clone(), dir, context.clone());

        let project_folder = project_folder.join("templates");
        let log = Log {};

        tera.register_filter("sort_object", sort_object);
        tera.register_filter("markdown", markdown_filter);
        tera.register_filter("include_file", include_file.clone());
        tera.register_filter("log", log);

        tera.register_function(
            "include_file",
            move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
                let file = if let Some(file) = args.get("file") {
                    if file.is_string() {
                        file
                    }else{
                        return Err(format!("invalid `file` ({file:?}) argument").into())
                    }
                }else{
                    return Err("`file` argument is missing".into())
                };

                let value = include_file.filter(file, args)?;
                Ok(value)
            },
        );
        let project_folder_ = project_folder.clone();
        tera.register_function(
            "markdown",
            move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
                let value = markdown(&project_folder_, args)?;
                Ok(value)
            },
        );
        tera.register_function(
            "read_md_files",
            move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
                let value = read_md_files(&project_folder, args)?;
                Ok(value)
            },
        );

        log_trace!("Render", "processing folders");

        let mut folders = HashSet::new();
        if section.is_none() {
            for template in tera.get_template_names() {
                // let target_file = self.ctx.target_folder.join(template);
                let folder = Path::new(&template);
                if let Some(parent) = folder.parent() {
                    if exclude.is_match(parent.to_str().unwrap()) {
                        continue;
                    }
                    if parent.to_string_lossy().len() != 0 {
                        folders.insert(parent.to_path_buf());
                    }
                }
            }
        }

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

        log_trace!("Render", "rendering");
        
        if let Some(section) = &section {
            for template in tera.get_template_names() {
                //println!("template: {template:?}, section.template:{:?}", section.template);
                if section.template.ends_with(template){
                    let sep = format!("/{}/", section.name);
                    for file in &section.files {
                        context.insert("section_file", file);
                        let mut parts = file.split(&sep);
                        parts.next();
                        let destination  = match parts.next(){
                            Some(value)=>PathBuf::from(&format!("{}/{}", section.name, value)).with_extension("html"),
                            None=>continue
                        };
                        //println!("destination: {destination:?}");
                        //println!("section_file: {file:?}");
                        
                        for (url_prefix, folder, language) in &info {
                            let content =
                                self.render_template(&tera, template, &mut context, language, url_prefix)?;
                            self.save_file(&content, destination.to_str().unwrap(), folder.as_ref()).await?;
                        }
                    }
                    break;
                }
            }
        } else {
            for template in tera.get_template_names() {
                if is_hidden(template) {
                    continue;
                }
                if exclude.is_match(template) {
                    log_trace!("Render", "{} `{}`", style("ignore:").yellow(), template);
                    continue;
                }

                for (url_prefix, folder, language) in &info {
                    let content =
                        self.render_template(&tera, template, &mut context, language, url_prefix)?;
                    self.save_file(&content, template, folder.as_ref()).await?;
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
        self.save_file(&content, "index.html", None).await?;

        Ok(())
    }

    pub async fn execute(&self) -> Result<()> {
        self.ctx.clean().await?;
        self.ctx.ensure_folders().await?;

        let glob = "templates/**/*{.html,.js,.raw}";
        let include = Filter::new(&[glob]);

        let settings = self.ctx.settings();

        let mut exclude_list = if let Some(ignore) = &settings.ignore {
            let mut list = ignore.iter().map(|s| s.as_str()).collect::<Vec<_>>();
            list.push("partial*");
            list.push("__INDEX__.html");

            list
        } else {
            vec!["partial*", "__INDEX__.html"]
        };

        let mut exclude = Filter::new(&exclude_list);

        // render sections
        if let Some(sections) = &self.ctx.manifest.sections {
            log_trace!("Render", "loading sections");
            let mut list = vec![];
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

                if !template.ends_with(".html"){
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

                        Some(format!("../../{}", relative.to_str().unwrap().to_string()))
                    })
                    .collect::<Vec<String>>();
                //let template_path = PathBuf::from(&template).with_file_name("*");
                //let tpl = template_path.to_str().unwrap().to_string();
                let tpl = "templates/**/*.html".to_string();
                //println!("###### tpl: {tpl:?}");
                let section_info = SectionInfo {
                    name: name.clone(),
                    template: template.clone(),
                    files
                };
                let mut p = template.split("templates/");
                p.next();
                let p = p.next().unwrap();
                section_exclude_list.push(p);
                list.push((section_info, tpl));
            }

            for a in section_exclude_list{
                exclude_list.push(a);
            }

            exclude = Filter::new(&exclude_list);

            //println!("exclude: {exclude:#?}");

            for (section_info, tpl) in list{
                self.render(&tpl, &exclude, &settings, Some(section_info))
                    .await?;
            }
        }

        //

        let render_start = Instant::now();
        log_trace!("Migrate", "migrating files");
        self.migrate(&include, &exclude).await?;
        log_trace!("Render", "loading templates");
        self.render(glob, &exclude, &settings, None).await?;

        

        let duration = render_start.elapsed();
        log_info!(
            "Build",
            "rendering complete in {} msec",
            duration.as_millis()
        );

        let package_json = self.ctx.site_folder.join("package.json");
        let node_modules = self.ctx.site_folder.join("node_modules");
        if package_json.is_file() && !node_modules.is_dir() {
            cmd!("npm", "install").dir(&self.ctx.site_folder).run()?;
        }

        Ok(())
    }
}
