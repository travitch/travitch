use serde_json::json;
use serde_json::value::{Map, Value as Json};
use std::fs::File;
use handlebars::{Handlebars, Helper, Context, Output, RenderContext, RenderError};
use graphql_client::{GraphQLQuery, Response};
use reqwest::blocking;

type URI = String;
type DateTime = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path="data/schema.docs.graphql",
    query_path="data/query.graphql",
    response_derives = "Debug"
)]
struct MyRepositoriesQuery;

/// Execute the github repository query in data/query.graphql
fn query_github(github_api_token : &str) -> anyhow::Result<my_repositories_query::ResponseData> {
    let vars = my_repositories_query::Variables {
    };
    let request_body = MyRepositoriesQuery::build_query(vars);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, reqwest::header::HeaderValue::from_str(&format!("Bearer {}", github_api_token)).unwrap());
    // Note that setting the user agent is *critical*, as Github rejects queries without one
    let client = blocking::ClientBuilder::new().user_agent("graphql-rust/0.10.0").build()?;

    let res = client.post("https://api.github.com/graphql").json(&request_body).headers(headers).send()?;
    let response_body: Response<my_repositories_query::ResponseData> = res.json()?;
    response_body.data.ok_or(anyhow::anyhow!("No data in query response"))
}

struct Repository {
    id: String,
    created_at: String,
    description: Option<String>,
    fork_count: i64,
    stargazer_count: i64,
    languages: Vec<String>,
    name: String,
    updated_at: String,
    url: String
}

/// A trait for translating the various individual auto-generated repository
/// types into a canonical one that is easier to process
trait AsRepository {
    fn as_repository(&self) -> Repository;
}

impl AsRepository for my_repositories_query::MyRepositoriesQueryOrganizationRepositoriesEdgesNode {
    fn as_repository(&self) -> Repository{
        Repository {
            id: self.id.clone(),
            created_at: self.created_at.clone(),
            description: self.description.clone(),
            fork_count: self.fork_count,
            stargazer_count: self.stargazer_count,
            languages: Vec::new(),
            name: self.name.clone(),
            updated_at: self.updated_at.clone(),
            url: self.url.clone()
        }
    }
}

impl AsRepository for my_repositories_query::MyRepositoriesQueryUserRepositoriesEdgesNode {
    fn as_repository(&self) -> Repository{
        Repository {
            id: self.id.clone(),
            created_at: self.created_at.clone(),
            description: self.description.clone(),
            fork_count: self.fork_count,
            stargazer_count: self.stargazer_count,
            languages: Vec::new(),
            name: self.name.clone(),
            updated_at: self.updated_at.clone(),
            url: self.url.clone()
        }
    }
}

impl AsRepository for my_repositories_query::MyRepositoriesQueryRepository {
    fn as_repository(&self) -> Repository{
        Repository {
            id: self.id.clone(),
            created_at: self.created_at.clone(),
            description: self.description.clone(),
            fork_count: self.fork_count,
            stargazer_count: self.stargazer_count,
            languages: Vec::new(),
            name: self.name.clone(),
            updated_at: self.updated_at.clone(),
            url: self.url.clone()
        }
    }
}

fn add_json_repo_entry(data : &mut Map<String, Json>, repo_node : &Repository) {
    let desc = repo_node.description.as_ref().map_or("(No description)".to_string(), |d| d.clone());
    let payload = json!({
        "name": repo_node.name.clone(),
        "created_at": repo_node.created_at.clone(),
        "description": desc,
        "url": repo_node.url.clone(),
        "fork_count": repo_node.fork_count,
        "star_count": repo_node.stargazer_count
    });
    data.insert(repo_node.name.clone(), payload);
}

/// Unpack the raw response data, which has lots of internal option nodes, into
/// a simple and flatter structure that can be processed automatically in
/// handlebars templates
fn make_data(raw_data : my_repositories_query::ResponseData) -> Map<String, Json> {
    let mut data = Map::new();

    // First, extract the repositories attached to the github user
    let user = raw_data.user.expect("Missing user query results");
    let user_repos_edges = user.repositories.edges.expect("Missing user repository edges");
    for repo in user_repos_edges.iter() {
        let repo_node = repo.as_ref().expect("Expected repository edge").node.as_ref().expect("Expected repository node");
        add_json_repo_entry(&mut data, &repo_node.as_repository());
    }

    let galois = raw_data.organization.expect("Missing organization query results");
    let galois_repos_edges = galois.repositories.edges.expect("Missing galois repository edges");
    for repo in galois_repos_edges.iter() {
        let repo_node = repo.as_ref().expect("Expected repository edge").node.as_ref().expect("Expected repository node");
        add_json_repo_entry(&mut data, &repo_node.as_repository());
    }

    let taffybar = raw_data.repository.expect("Missing taffybar query results");
    add_json_repo_entry(&mut data, &taffybar.as_repository());
    data
}

/// Extract the given key as a string, throwing an error if it isn't really one
fn context_string<'a>(obj : &'a Map<String, Json>, key : &str) -> &'a str {
    obj.get(key).unwrap().as_str().unwrap()
}

/// Extract the given key as a number, throwing an error if it isn't really one
fn context_number(obj : &Map<String, Json>, key : &str) -> i64 {
    obj.get(key).unwrap().as_i64().unwrap()
}

/// Render a repository object as a string
fn render_repo_object(obj_map: &Map<String, Json>) -> String {
    format!("[{}]({}) [{} :star:]: {}",
            context_string(&obj_map, "name"),
            context_string(&obj_map, "url"),
            context_number(&obj_map, "star_count"),
            context_string(&obj_map, "description")
    )
}

fn render_repo_object_hackage(obj_map: &Map<String, Json>, hackage_url: &str) -> String {
    format!("[{}]({}) [{} :star: [:book:]({})]: {}",
            context_string(&obj_map, "name"),
            context_string(&obj_map, "url"),
            context_number(&obj_map, "star_count"),
            hackage_url,
            context_string(&obj_map, "description")
    )
}


/// Handle the "render_repo" directive in templates
///
/// This emits HTML based on the data returned from github (stored in the
/// context object, which is a JSON map)
fn render_repo_helper(
    h: &Helper,
    _: &Handlebars,
    ctx: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> Result<(), RenderError> {
    let param = h
        .param(0)
        .ok_or(RenderError::new("Param 0 is required for the `render_repo` helper."))?;
    let repo_name = param.value().as_str().expect("Expected parameter to `render_repo` to be a string");
    let repo_map = ctx.data();
    match repo_map.as_object().expect("Repository context object should be a `Map`").get(repo_name) {
        None => {
            Err(RenderError::new(format!("Expected repository for {}", repo_name)))
        },
        Some(obj) => {
            let obj_map = obj.as_object().unwrap();
            let rendered = render_repo_object(obj_map);
            out.write(rendered.as_ref())?;
            Ok(())
        }
    }
}

fn render_repo_hackage_helper(
    h: &Helper,
    _: &Handlebars,
    ctx: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> Result<(), RenderError> {
    let name_param = h
        .param(0)
        .ok_or(RenderError::new("Param 0 is required for the `render_repo_hackage` helper."))?;
    let hackage_url_param = h
        .param(1)
        .ok_or(RenderError::new("Param 1 is required for the `render_repo_hackage` helper."))?;
    let hackage_url = hackage_url_param.value().as_str().expect("Expected parameter to `render_repo_hackage` to be a string");
    let repo_name = name_param.value().as_str().expect("Expected parameter to `render_repo_hackage` to be a string");
    let repo_map = ctx.data();
    match repo_map.as_object().expect("Repository context object should be a `Map`").get(repo_name) {
        None => {
            Err(RenderError::new(format!("Expected repository for {}", repo_name)))
        },
        Some(obj) => {
            let obj_map = obj.as_object().unwrap();
            let rendered = render_repo_object_hackage(obj_map, hackage_url);
            out.write(rendered.as_ref())?;
            Ok(())
        }
    }
}

fn main() -> anyhow::Result<(), anyhow::Error> {
    let github_api_token = std::env::var("GITHUB_API_TOKEN").expect("Missing GITHUB_API_TOKEN env var");
    let raw_data = query_github(&github_api_token)?;
    let normalized_data = make_data(raw_data);
    let mut handlebars = Handlebars::new();
    // FIXME: Add a version of this helper that also accepts a hackage link
    handlebars.register_helper("render_repo", Box::new(render_repo_helper));
    handlebars.register_helper("render_repo_hackage", Box::new(render_repo_hackage_helper));
    handlebars.register_template_file("README", "./data/template/README.hbs")?;
    let mut output_file = File::create("README.md")?;
    handlebars.render_to_write("README", &normalized_data, &mut output_file)?;
    Ok(())
}
