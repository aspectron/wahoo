use crate::prelude::*;
use std::collections::{HashMap, HashSet};
use walkdir::WalkDir;
use workflow_i18n::Dict;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Language {
    locale: String,
    name: String,
}

pub struct Builder {
    ctx: Arc<Context>,
    i18n_dict: Dict,
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
        let list = WalkDir::new(&self.ctx.project_folder)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let relative = path.strip_prefix(&self.ctx.project_folder).unwrap();

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
            std::fs::create_dir_all(self.ctx.target_folder.join(folder))?;
        }

        for file in list {
            let to_file = self.ctx.target_folder.join(&file);
            // println!("+{}",file.display());
            log_trace!(
                "Migrate",
                "{} `{}` to `{}`",
                style("migrate:").cyan(),
                file.display(),
                to_file.display()
            );
            std::fs::copy(self.ctx.project_folder.join(&file), to_file)?;
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
            self.ctx.target_folder.join(language).join(template)
        } else {
            self.ctx.target_folder.join(template)
        };
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

        log_info!(
            "Rendering",
            "{}",
            style(format!("{url_prefix}{}", template)).blue()
        );
        match tera.render(template, context) {
            Ok(s) => Ok(s),
            Err(e) => {
                let mut cause = e.source();
                while let Some(e) = cause {
                    println!();
                    log_error!("{}", e);
                    cause = e.source();
                }

                Err(e.into())
            }
        }
    }

    /// Render templates into the target directory
    pub async fn render(&self, glob: &str, exclude: &Filter, settings: &Settings) -> Result<()> {
        let project_folder = self.ctx.project_folder.clone();
        let dir = self.ctx.project_folder.join(glob);
        let dir = dir.to_str().unwrap();
        let mut tera = match tera::Tera::new(dir) {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}, glob:{}", e, glob);
                ::std::process::exit(1);
            }
        };

        let mut context = tera::Context::from_serialize(&self.ctx.manifest.toml)?;

        let sort_object = SortObject {};
        let markdown_filter = Markdown {};

        let include_file = IncludeFile::new(project_folder.clone(), dir, context.clone());

        let project_folder = project_folder.join("templates");
        let log = Log{};

        tera.register_filter("sort_object", sort_object);
        tera.register_filter("markdown", markdown_filter);
        tera.register_filter("include_file", include_file);
        tera.register_filter("log", log);
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

        //let table = self.ctx.manifest.toml.as_table().unwrap();
        //context.insert("table", table);
        // let mut context = tera::Context::new();
        //context.insert("username", &"Bob");
        //println!("context: {:?}", context);
        // context.insert("numbers", &vec![1, 2, 3]);
        // context.insert("show_all", &false);
        // context.insert("bio", &"<script>alert('pwnd');</script>");

        log_trace!("Render", "processing folders");

        let mut folders = HashSet::new();
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
                self.ctx.target_folder.join(language)
            } else {
                self.ctx.target_folder.clone()
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

        log_info!("Render", "rendering");
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

        let exclude = if let Some(ignore) = &settings.ignore {
            let mut list = ignore.iter().map(|s| s.as_str()).collect::<Vec<_>>();
            list.push("partial*");
            list.push("__INDEX__.html");

            Filter::new(&list)
        } else {
            let list = vec!["partial*", "__INDEX__.html"];
            Filter::new(&list)
        };

        log_trace!("Migrate", "migrating files");
        self.migrate(&include, &exclude).await?;
        log_trace!("Render", "loading templates");
        self.render(glob, &exclude, &settings).await?;
        log_info!("Build", "done");

        Ok(())
    }
}
