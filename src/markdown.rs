
use pulldown_cmark::{
    Parser, Tag, Event,
    escape::{escape_html, escape_href},
    LinkType, CowStr, Options, html
};
//use workflow_log::log_trace;

pub fn markdown_to_html(str:&str)->String{

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&str, options);

    let parser = parser.map(|event| match event {
        //Event::Text(text) => Event::Text(text.replace("abbr", "abbreviation").into()),
        Event::Start(tag)=>{
            let t = match tag{
                Tag::Link(link_type, dest, title)=>{
                    //log_trace!("link-type: {:?}, href:{:?}, title:{:?}", link_type, dest, title);
                    let mut prefix = "";
                    if link_type.eq(&LinkType::Email){
                        prefix = "mailto:";
                    }

                    let mut href = String::new();
                    let mut dest_str = dest.into_string();
                    let _ = escape_href(&mut href, &mut dest_str);
                    let href = CowStr::from(href);
                    if title.is_empty() {
                        return Event::Html(
                            CowStr::from(format!("<a target=\"_blank\" href=\"{}{}\">", CowStr::from(prefix), href))
                        );
                    }else{
                        let mut title_ = String::new();
                        let mut title_str = title.into_string();
                        let _ = escape_html(&mut title_, &mut title_str);
                        let title = CowStr::from(title_);
                        return Event::Html(
                            CowStr::from(format!("<a target=\"_blank\" href=\"{}{}\" title=\"{}\">", prefix, href, title))
                        );
                    }
                }
                _=>{
                    tag
                }
            };
            Event::Start(t)
        },
        _ => event
    });

    // Write to String buffer.
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}
