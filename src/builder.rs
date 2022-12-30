use std::collections::{HashMap, HashSet};
use crate::prelude::*;
use walkdir::WalkDir;

pub struct Builder {
    ctx : Arc<Context>,
}

impl Builder {
    pub fn new(ctx: Arc<Context>) -> Builder {
        Builder {
            ctx
        }
    }

    /// Migrate non-template files into the target directory
    pub async fn migrate(&self, include : &Filter, exclude : &Filter) -> Result<()> {

        let list = WalkDir::new(&self.ctx.project_folder)
            .into_iter()
            .flatten()
            .filter_map(|entry|{
                let path = entry.path();
                let relative = path.strip_prefix(&self.ctx.project_folder).unwrap();
                
                let relative_str = relative.to_str().unwrap();
                if relative_str.len() == 0 || is_hidden(relative) {
                    return None;
                }

                if include.is_match(relative_str) || !path.is_file() {
                    None
                } else if exclude.is_match(relative_str) {
                    log_trace!("Migrate","{} `{}`",style("ignore:").yellow(),path.display());
                    None
                } else {
                    Some(Path::new(relative).to_path_buf())
                }
            });

        let mut folders = HashSet::new();
        let list: Vec::<_> = list.collect();
        for path in list.iter() {
            if let Some(folder) = path.parent() {
                if folder.to_string_lossy().len() != 0 {
                    folders.insert(folder.to_path_buf());
                }
            }
        }
    
        for folder in folders {
            log_trace!("Migrate","folder `{}`", folder.display());
            std::fs::create_dir_all(self.ctx.target_folder.join(folder))?; 
        }
    
        for file in list {
            let to_file = self.ctx.target_folder.join(&file);
            // println!("+{}",file.display());
            log_trace!("Migrate","{} `{}` to `{}`",style("migrate:").cyan(), file.display(), to_file.display());
            std::fs::copy(self.ctx.project_folder.join(&file),to_file)?;
        }

        Ok(())
    }

    /// Render templates into the target directory
    pub async fn render(&self, glob : &str, exclude: &Filter) -> Result<()> {
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

        let context = tera::Context::from_serialize(&self.ctx.manifest.toml)?;

        let sort_object = SortObject{};
        let markdown_filter = Markdown{};

        let include_file = IncludeFile::new(
            project_folder.clone(),
            dir,
            context.clone()
        );

        let project_folder = project_folder.join("templates");

        tera.register_filter("sort_object", sort_object);
        tera.register_filter("markdown", markdown_filter);
        tera.register_filter("include_file", include_file);
        tera.register_function("markdown", move |args: &HashMap<String, tera::Value>|->tera::Result<tera::Value>{
            let value = markdown(&project_folder, args)?;
            Ok(value)
        });


        

        //context.insert("_tera", &tera);
        //tera.register_function("include_file", include_file);
        
        //println!("context.get(\"project\"): {:#?}", context.get("project"));
        //let table = self.ctx.manifest.toml.as_table().unwrap();
        //context.insert("table", table);
        // let mut context = tera::Context::new();
        //context.insert("username", &"Bob");
        //println!("context: {:?}", context);
        // context.insert("numbers", &vec![1, 2, 3]);
        // context.insert("show_all", &false);
        // context.insert("bio", &"<script>alert('pwnd');</script>");

        log_trace!("Render","processing folders");

        let mut folders = HashSet::new();
        for template in tera.get_template_names() {
            // let target_file = self.ctx.target_folder.join(template);
            let folder = Path::new(&template);
            if let Some(parent) = folder.parent() {
                if parent.to_string_lossy().len()!= 0 {
                    folders.insert(parent.to_path_buf());
                }
            }
        }
    
        for folder in folders {
            log_trace!("Folders","{} `{}`",style("creating:").cyan(),folder.display());
            std::fs::create_dir_all(self.ctx.target_folder.join(folder))?; 
        }

        log_info!("Render","rendering");
        for template in tera.get_template_names() {

            if is_hidden(template) {
                continue;
            }

            if exclude.is_match(template) {
                log_trace!("Render","{} `{}`", style("ignore:").yellow(),template);
                continue;
            }

            use std::error::Error;
            log_info!("Rendering", "{}", style(template).blue());
            match tera.render(template, &context) {
                Ok(s) => {
                    let target_file = self.ctx.target_folder.join(template);
                    log_trace!("Render","{} `{}`", style("render:").cyan(), template);
                    fs::write(target_file,&s).await?;
                },
                Err(e) => {
                    let mut cause = e.source();
                    while let Some(e) = cause {
                        println!("");
                        log_error!("{}",e);
                        cause = e.source();
                    }

                    return Err(e.into());
                }
            };
        }

        Ok(())
    }

    pub async fn execute(&self) -> Result<()> {

        self.ctx.clean().await?;
        self.ctx.ensure_folders().await?;

        let glob = "templates/**/*{.html,.js,.raw}";
        let include = Filter::new(&[glob]);
        let exclude = if let Some(Settings { ignore : Some(ignore) }) = &self.ctx.manifest.settings {
            let mut list = ignore.iter().map(|s|s.as_str()).collect::<Vec<_>>();
            list.push("partial/**/*");

            Filter::new(&list)
        } else {
            let list = vec!["partial/**/*"];
            Filter::new(&list)
        };

        log_trace!("Migrate","migrating files");
        self.migrate(&include,&exclude).await?;
        log_trace!("Render","loading templates");
        self.render(glob, &exclude).await?;
        log_info!("Build","done");
        println!("");

        Ok(())
    }



}

