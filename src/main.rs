use sailfish::TemplateOnce;

#[derive(TemplateOnce)]
#[template(path = "index.stpl")]
struct HelloTemplate {
    messages: Vec<String>
}

fn main() {
    let ctx = HelloTemplate {
        messages: vec![String::from("foo"), String::from("bar")],
    };
    println!("{}", ctx.render_once().unwrap());
}