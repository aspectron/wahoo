use std::collections::HashSet;
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
    pub async fn render(&self, glob : &str, exclude: &Filter) -> Result<()> {

        let tera = match tera::Tera::new(self.ctx.project_folder.join(glob).to_str().unwrap()) {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        };

        let context = tera::Context::from_serialize(&self.ctx.manifest.toml)?;
        
        // let mut context = tera::Context::new();
        //context.insert("username", &"Bob");
        //println!("context: {:?}", context);
        // context.insert("numbers", &vec![1, 2, 3]);
        // context.insert("show_all", &false);
        // context.insert("bio", &"<script>alert('pwnd');</script>");

        log_info!("Render","processing folders");

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
        log_info!("Project", "`{}`", self.ctx.project_folder.display());

        let glob = "**/*{.html,.js}";
        let include = Filter::new(&[glob]);
        let exclude = if let Some(Settings { ignore : Some(ignore) }) = &self.ctx.manifest.settings {
            let list = ignore.iter().map(|s|s.as_str()).collect::<Vec<_>>();
            Filter::new(&list)
        } else {
            Filter::default()
        };

        log_info!("Migrate","migrating files");
        self.migrate(&include,&exclude).await?;
        log_info!("Render","loading templates");
        self.render(glob, &exclude).await?;
        log_info!("Build","done");
        println!("");

        Ok(())
    }



}

