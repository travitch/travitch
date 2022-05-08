use serde_json::value::{Map, Value as Json};
use std::fs::File;
use handlebars::{ Handlebars };

fn make_data() -> Map<String, Json> {
    let mut data = Map::new();

    data
}

fn main() -> anyhow::Result<(), anyhow::Error> {
    let data = make_data();
    let mut handlebars = Handlebars::new();
    handlebars.register_template_file("README", "./data/template/README.hbs")?;
    let mut output_file = File::create("README.md")?;
    handlebars.render_to_write("README", &data, &mut output_file)?;
    Ok(())
}
