
use pulldown_cmark::{
    Parser, Tag, Event,
    escape::{escape_html, escape_href},
    LinkType, CowStr, Options, html
};
//use workflow_log::log_trace;
pub fn parse_toml_from_markdown(str:&str)->Option<String>{

    let options = Options::empty();
    let parser = Parser::new_ext(str, options);

    let mut result = None;
    let mut buffer = String::new();

    let mut comment_started = false;
    let parser = parser.map(|event| match event {
        Event::Html(code)=>{
            if code.starts_with("-->"){
                if comment_started{
                    result = Some(buffer.clone());
                }
                comment_started = true;
                Event::Html(CowStr::Borrowed(""))
            }else if comment_started{
                buffer.push_str(&code.to_string());
                Event::Html(CowStr::Borrowed(""))
            }else if code.starts_with("<!---toml"){
                comment_started = true;
                Event::Html(CowStr::Borrowed(""))
            }else{
                Event::Html(code)
            }
        }
        _ => {
            //println!("event: {:?}", event);
            event
        }
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    result
}

pub fn markdown_to_html(str:&str, open_external_in_new_window:bool)->String{

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(str, options);

    //println!("markdown_to_html: {str}");
    let mut comment_started = false;
    let parser = parser.map(|event| match event {
        //Event::Text(text) => Event::Text(text.replace("abbr", "abbreviation").into()),
        
        Event::Start(tag)=>{
            let t = match tag{
                Tag::Link(link_type, dest, title)=>{
                    //log_trace!("link-type: {:?}, href:{:?}, title:{:?}", link_type, dest, title);
                    let mut dest_str = dest.into_string();
                    let mut href_str = String::new();
                    let _ = escape_href(&mut href_str, &mut dest_str);
                    href_str = href_str.trim().to_string();
                    
                    let mut new_window = false;

                    let mut prefix = "";
                    if link_type.eq(&LinkType::Email){
                        prefix = "mailto:";
                    }else if open_external_in_new_window && href_str.starts_with("http"){
                        new_window = true;
                    }

                    let href = CowStr::from(href_str);
                    if title.is_empty() {
                        if new_window{
                            return Event::Html(
                                CowStr::from(format!("<a target=\"_blank\" href=\"{}{}\">", CowStr::from(prefix), href))
                            );
                        }else{
                            return Event::Html(
                                CowStr::from(format!("<a href=\"{}{}\">", CowStr::from(prefix), href))
                            );
                        }
                        
                    }else{
                        let mut title_ = String::new();
                        let mut title_str = title.into_string();
                        let _ = escape_html(&mut title_, &mut title_str);
                        let title = CowStr::from(title_);
                        if new_window{
                            return Event::Html(
                                CowStr::from(format!("<a target=\"_blank\" href=\"{}{}\" title=\"{}\">", prefix, href, title))
                            );
                        }else{
                            return Event::Html(
                                CowStr::from(format!("<a href=\"{}{}\" title=\"{}\">", prefix, href, title))
                            );
                        }
                    }
                }
                _=>{
                   // println!("tag: {:?}", tag);
                    tag
                }
            };
            Event::Start(t)
        }
        
        Event::Html(code)=>{
            if code.starts_with("-->"){
                comment_started = false;
                Event::Html(CowStr::Borrowed(""))
            }else if comment_started{
                Event::Html(CowStr::Borrowed(""))
            }else if code.starts_with("<!---"){
                comment_started = true;
                Event::Html(CowStr::Borrowed(""))
            }else{
                Event::Html(code)
            }
        }
        _ => {
            //println!("event: {:?}", event);
            event
        }
    });

    // Write to String buffer.
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}
